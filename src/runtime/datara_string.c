#if defined(__APPLE__)
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE 1
#endif
#else
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <errno.h>
#include "datara_runtime.h"
#include "datara_rt_internal.h"

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#include <dbghelp.h>
#else
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/mman.h>
#ifndef MAP_ANONYMOUS
#ifdef MAP_ANON
#define MAP_ANONYMOUS MAP_ANON
#endif
#endif
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <unistd.h>
#include <fcntl.h>
#include <time.h>
#include <pthread.h>
#include <sched.h>
#include <dirent.h>
#if defined(__has_include)
#if __has_include(<execinfo.h>)
#include <execinfo.h>
#define DATARA_HAS_EXECINFO 1
#endif
#elif !defined(__musl__)
#include <execinfo.h>
#define DATARA_HAS_EXECINFO 1
#endif
#endif

#ifndef INADDR_NONE
#define INADDR_NONE ((unsigned long)0xffffffff)
#endif

#define DATARA_SSO_MAX_LEN 22
#define DATARA_SSO_RING_COUNT 512
#define DATARA_SSO_SLOT_SIZE 24

static DATARA_TLS char tls_sso_ring[DATARA_SSO_RING_COUNT][DATARA_SSO_SLOT_SIZE];
static DATARA_TLS uint32_t tls_sso_idx = 0;

const char* datara_rt_str_sso(const char* s) {
    if (!s) return "";
    size_t len = strlen(s);
    if (len <= DATARA_SSO_MAX_LEN) {
        uint32_t idx = (tls_sso_idx++) % DATARA_SSO_RING_COUNT;
        char* slot = tls_sso_ring[idx];
        memcpy(slot, s, len);
        slot[len] = '\0';
        return slot;
    }
    tls_heap_alloc_count++;
    char* buf = (char*)malloc(len + 1);
    if (!buf) return "";
    memcpy(buf, s, len);
    buf[len] = '\0';
    return buf;
}

int64_t datara_rt_str_is_sso(const char* s) {
    if (!s) return 1;
    return strlen(s) <= DATARA_SSO_MAX_LEN ? 1 : 0;
}

static DATARA_TLS char* tls_scratch_ring = NULL;
static DATARA_TLS size_t tls_scratch_offset = 0;
// Payload of the most recent heap growable block (P1: heap fallback keeps
// growable status). Only this exact pointer is vouched for outside the ring,
// so recognition stays O(1) and sound; older heap blocks are garbage the
// runtime intentionally never frees (same leak-by-design as the scratch ring).
static DATARA_TLS char* tls_heap_recent_payload = NULL;

// Growable-ownership header magic (P1). Must precede datara_str_scratch_alloc,
// which stamps every new block with the dead magic.
#define DATARA_STRBUF_MAGIC_LIVE 0x44545253u /* "SRTD" */
#define DATARA_STRBUF_MAGIC_DEAD 0x54525344u /* "DRST" */
#define DATARA_STRBUF_HDR_BYTES 16 /* min scratch granularity */

typedef struct {
    uint32_t magic;
    uint32_t cap;
    uint32_t len; /* cached strlen of the payload (P1: O(1) appends) */
    uint32_t pad;
} DataraStrBufHdr;

// Heap fallback block with the SAME growable header layout as scratch blocks.
// A concat chain that outgrows the scratch ring (or allocates a >half-ring
// block) continues on the heap WITHOUT losing its growable status: the header
// lives in a 32-byte malloc'ed region (16 header + 16 data) so the in-place
// append fast path keeps working for long chains. Previously this path
// returned a bare malloc block whose header read as dead magic, silently
// demoting the whole remaining chain to the quadratic copy path (the
// "sometimes 1.5x, sometimes 1300x" heuristic-vs-C flakiness).
static char* datara_str_heap_alloc_growable(size_t len) {
    tls_heap_alloc_count++;
    char* hdr = (char*)malloc(len + 1 + DATARA_STRBUF_HDR_BYTES * 2);
    if (!hdr) return NULL;
    // Align the 16-byte header to the malloc boundary; DATARA_STRBUF_HDR_BYTES
    // offset keeps block+16 16-aligned because malloc returns >=16-aligned
    // pointers on both Windows and glibc/musl.
    char* p = hdr + DATARA_STRBUF_HDR_BYTES;
    ((DataraStrBufHdr*)p)->magic = DATARA_STRBUF_MAGIC_DEAD;
    ((DataraStrBufHdr*)p)->cap = 0;
    ((DataraStrBufHdr*)p)->len = 0;
    ((DataraStrBufHdr*)p)->pad = 0;
    return p + DATARA_STRBUF_HDR_BYTES;
}

char* datara_str_scratch_alloc(size_t len) {
    if (!tls_scratch_ring) {
        tls_scratch_ring = (char*)malloc(DATARA_SCRATCH_RING_SIZE);
        if (!tls_scratch_ring) return datara_str_heap_alloc_growable(len);
    }
    // P1: every block reserves DATARA_STRBUF_HDR_BYTES for the optional
    // growable-ownership header so the header NEVER overlaps the previous
    // block's data. Blocks stay 16-aligned, so block+16 is 16-aligned too.
    size_t needed = ((len + 1 + 15) & ~15) + DATARA_STRBUF_HDR_BYTES;
    if (needed > (DATARA_SCRATCH_RING_SIZE / 2)) {
        // P1: cap at half the ring so one huge block cannot starve other
        // scratch users. A grow chain that outgrows this bound continues on
        // the heap WITH its growable header, so appends keep the amortized
        // O(1) path instead of degrading to quadratic copies.
        return datara_str_heap_alloc_growable(len);
    }
    if (tls_scratch_offset + needed >= DATARA_SCRATCH_RING_SIZE) {
        // Ring exhausted: continue on the heap instead of wrapping around,
        // which would silently overwrite strings that may still be live.
        // The heap fallback carries the growable header so concat chains do
        // not lose in-place-append status mid-loop.
        return datara_str_heap_alloc_growable(len);
    }
    char* hdr = &tls_scratch_ring[tls_scratch_offset];
    tls_scratch_offset += needed;
    char* p = hdr + DATARA_STRBUF_HDR_BYTES;
    ((DataraStrBufHdr*)hdr)->magic = DATARA_STRBUF_MAGIC_DEAD;
    ((DataraStrBufHdr*)hdr)->cap = 0;
    ((DataraStrBufHdr*)hdr)->len = 0;
    ((DataraStrBufHdr*)hdr)->pad = 0;
    return p;
}

int datara_str_in_scratch_ring(const void* p) {
    if (!tls_scratch_ring || !p) return 0;
    return ((const char*)p >= tls_scratch_ring && (const char*)p < tls_scratch_ring + DATARA_SCRATCH_RING_SIZE);
}

// ============================================================================
// Growable (unique-owner) string buffers — P1 fix for the O(n^2) concat loop.
//
// Scratch-allocated buffers carry a hidden 16-byte header right before the
// returned char* (space is reserved by datara_str_scratch_alloc for every
// block):
//     [DATARA_STRBUF_MAGIC:i32][capacity:i32][length:i32][pad:i32]
// A buffer whose header is stamped with DATARA_STRBUF_MAGIC_LIVE is exclusively
// owned by the one DMIR value that carries its pointer — the optimizer only
// emits datara_rt_str_concat_mut when it PROVED the previous LHS value dead
// (use-count == 1), so in-place growth can never corrupt an alias. Legacy code
// paths and FFI callers hand over plain malloc/SSO pointers whose header reads
// as the dead magic and fall back to the classic copy path.
// ============================================================================

static inline int datara_strbuf_is_growable(const char* s) {
    if (!s) return 0;
    ptrdiff_t d = (const char*)s - tls_scratch_ring;
    if (d >= 0 && (size_t)d < DATARA_SCRATCH_RING_SIZE && (d & 15) == 0) {
        const DataraStrBufHdr* h = (const DataraStrBufHdr*)(s - DATARA_STRBUF_HDR_BYTES);
        return h->magic == DATARA_STRBUF_MAGIC_LIVE;
    }
    // Heap fallback blocks: only the most recent heap payload carries live
    // growable status (see tls_heap_recent_payload). Exact-pointer match is
    // the soundness bar — arbitrary strings can never accidentally qualify.
    if (s == tls_heap_recent_payload) {
        const DataraStrBufHdr* h = (const DataraStrBufHdr*)(s - DATARA_STRBUF_HDR_BYTES);
        return h->magic == DATARA_STRBUF_MAGIC_LIVE;
    }
    return 0;
}

// Mark a result buffer as exclusively owned by its producer. Called on the
// scratch/heap result of the copy concat right before returning it: if the
// very next consumer of this value is the proven-unique append (concat_mut),
// it may grow the buffer in place.
static inline void datara_strbuf_mark_live(char* buf, size_t cap, size_t len) {
    if (!buf) return;
    ptrdiff_t d = buf - tls_scratch_ring;
    if (d < 0 || (size_t)d >= DATARA_SCRATCH_RING_SIZE || (d & 15) != 0) {
        // Outside the ring: only scratch_alloc's heap fallback can legally get
        // here (every mark_live call site stamps a fresh scratch_alloc result).
        // Register it as the current heap growable block.
        tls_heap_recent_payload = buf;
    }
    if (cap < DATARA_STRBUF_HDR_BYTES) return;
    DataraStrBufHdr* h = (DataraStrBufHdr*)(buf - DATARA_STRBUF_HDR_BYTES);
    h->magic = DATARA_STRBUF_MAGIC_LIVE;
    h->cap = (uint32_t)cap;
    h->len = (uint32_t)len;
}

// Fresh uniquely-owned copy of a growable buffer. Used by the identity
// paths below: a concat must NEVER return an input pointer that is still
// referenced elsewhere, otherwise the optimizer's unique-ownership proof
// (and the in-place append it licenses) becomes unsound.
static const char* datara_strbuf_copy_growable(const char* s) {
    size_t ls;
    if (datara_strbuf_is_growable(s)) {
        ls = ((const DataraStrBufHdr*)(s - DATARA_STRBUF_HDR_BYTES))->len;
    } else {
        ls = strlen(s);
    }
    char* buf = datara_str_scratch_alloc(ls);
    if (!buf) return "";
    if (ls) datara_fast_copy(buf, s, ls);
    buf[ls] = '\0';
    datara_strbuf_mark_live(buf, (ls + 1 + 15) & ~(size_t)15, ls);
    return buf;
}

// Exclusive-owner append: dst must be a live growable scratch buffer (the
// optimizer proved the value unique); on success the old buffer is extended
// in place and its ownership magic consumed, so the result starts fresh.
const char* datara_rt_str_concat_mut(const char* dst, const char* b) {
    if (!b || b[0] == '\0') return dst ? dst : "";
    if (!dst || dst[0] == '\0') {
        // Identity / start of loop via empty dst: if b is already a growable buffer, return unique copy.
        if (b && datara_strbuf_is_growable(b)) return datara_strbuf_copy_growable(b);
        // Start of string building: allocate a live growable buffer in the scratch ring immediately.
        size_t lb = strlen(b);
        size_t init_cap = lb < 64 ? 64 : ((lb * 2 + 15) & ~15);
        char* buf = datara_str_scratch_alloc(init_cap);
        if (!buf) return b ? b : "";
        if (lb) datara_fast_copy(buf, b, lb);
        buf[lb] = '\0';
        datara_strbuf_mark_live(buf, ((init_cap + 1 + 15) & ~15), lb);
        return buf;
    }
    if (!datara_strbuf_is_growable(dst)) {
        return datara_rt_str_concat(dst, b); /* sound copy fallback */
    }
    size_t lb = strlen(b);
    DataraStrBufHdr* h = (DataraStrBufHdr*)(dst - DATARA_STRBUF_HDR_BYTES);
    /* P1: cached length instead of strlen(dst) — removes the O(n^2) rescan
       of the accumulated buffer on every append. */
    size_t la = h->len;
    size_t total = la + lb;
    size_t cap = h->cap;
    if (b >= dst && b < dst + la) {
        /* b aliases dst's payload (self-append variants): an in-place memcpy
           would read bytes we are concurrently overwriting. Take the sound
           copy path; the fresh result buffer is uniquely owned. */
        return datara_rt_str_concat(dst, b);
    }
    if (total + 1 > cap) {
        // Grow: check if dst is the most recent block at the tip of the scratch ring.
        size_t newcap = cap * 2;
        if (newcap < total + 1) newcap = total + 1;
        if (newcap < 64) newcap = 64;
        size_t old_aligned = (cap + 1 + 15) & ~15;
        size_t new_aligned = (newcap + 1 + 15) & ~15;
        char* hdr = (char*)h;
        if (tls_scratch_ring && (hdr + DATARA_STRBUF_HDR_BYTES + old_aligned == &tls_scratch_ring[tls_scratch_offset])
            && tls_scratch_offset + (new_aligned - old_aligned) < DATARA_SCRATCH_RING_SIZE) {
            tls_scratch_offset += (new_aligned - old_aligned);
            h->cap = (uint32_t)new_aligned;
            datara_fast_copy((char*)dst + la, b, lb);
            ((char*)dst)[total] = '\0';
            h->len = (uint32_t)total;
            return dst;
        }
        // Heap growable block: realloc preserves the header at the same
        // relative offset (malloc'ed base = dst - 2*HDR), so ownership stays
        // live and the chain keeps amortized O(1) appends past the ring.
        if (dst == tls_heap_recent_payload) {
            size_t need = total + 1 + DATARA_STRBUF_HDR_BYTES * 2;
            char* base = (char*)realloc((char*)h - DATARA_STRBUF_HDR_BYTES, need);
            if (!base) return "";
            h = (DataraStrBufHdr*)(base + DATARA_STRBUF_HDR_BYTES);
            char* ndst = base + DATARA_STRBUF_HDR_BYTES * 2;
            tls_heap_recent_payload = ndst;
            h->cap = (uint32_t)(need - 1 - DATARA_STRBUF_HDR_BYTES * 2 > newcap
                                    ? newcap
                                    : need - 1 - DATARA_STRBUF_HDR_BYTES * 2);
            datara_fast_copy((char*)ndst + la, b, lb);
            ((char*)ndst)[total] = '\0';
            h->len = (uint32_t)total;
            h->magic = DATARA_STRBUF_MAGIC_LIVE;
            return ndst;
        }
        char* nb = datara_str_scratch_alloc(newcap);
        if (!nb) return "";
        datara_fast_copy(nb, dst, la);
        datara_fast_copy(nb + la, b, lb);
        nb[total] = '\0';
        h->magic = DATARA_STRBUF_MAGIC_DEAD;
        datara_strbuf_mark_live(nb, (newcap + 1 + 15) & ~(size_t)15, total);
        return nb;
    }
    // In place: zero allocations, zero copies of the existing prefix.
    datara_fast_copy((char*)dst + la, b, lb);
    ((char*)dst)[total] = '\0'; /* dst is scratch-ring storage we own */
    h->len = (uint32_t)total; /* keep the cached length in sync */
    // Ownership stays LIVE: the optimizer's use-count==1 proof covers the
    // whole chain, so the next proven-unique append may keep growing here.
    return dst;
}

// Test surface for the refcount/aliasing semantics (P1 acceptance criteria).
int64_t datara_rt_strbuf_test_growable(const char* s) {
    return datara_strbuf_is_growable(s) ? 1 : 0;
}

int64_t datara_rt_strbuf_test_len(const char* s) {
    if (!datara_strbuf_is_growable(s)) return -1;
    const DataraStrBufHdr* h = (const DataraStrBufHdr*)(s - DATARA_STRBUF_HDR_BYTES);
    return (int64_t)h->len;
}

int64_t datara_rt_strbuf_test_cap(const char* s) {
    if (!datara_strbuf_is_growable(s)) return -1;
    ptrdiff_t d = (const char*)s - tls_scratch_ring;
    const DataraStrBufHdr* h = (const DataraStrBufHdr*)(s - DATARA_STRBUF_HDR_BYTES);
    (void)d;
    return (int64_t)h->cap;
}

void datara_rt_strbuf_test_consume(const char* s) {
    if (!s) return;
    ptrdiff_t d = (const char*)s - tls_scratch_ring;
    if (d < 0 || (size_t)d >= DATARA_SCRATCH_RING_SIZE || (d & 15) != 0) return;
    DataraStrBufHdr* h = (DataraStrBufHdr*)(s - DATARA_STRBUF_HDR_BYTES);
    h->magic = DATARA_STRBUF_MAGIC_DEAD;
}

// Ultra-fast zero-malloc string concatenation via thread-local circular bump allocator
const char* datara_rt_str_concat(const char* a, const char* b) {
    if (!a || a[0] == '\0') {
        // Identity paths: never return a still-referenced growable buffer
        // (unique-ownership invariant); copy it instead.
        if (b && datara_strbuf_is_growable(b)) return datara_strbuf_copy_growable(b);
        return b ? b : "";
    }
    if (!b || b[0] == '\0') {
        if (datara_strbuf_is_growable(a)) return datara_strbuf_copy_growable(a);
        return a ? a : "";
    }

    size_t la = strlen(a);
    size_t lb = strlen(b);
    size_t total = la + lb;
    if (total <= DATARA_SSO_MAX_LEN) {
        uint32_t idx = (tls_sso_idx++) % DATARA_SSO_RING_COUNT;
        char* slot = tls_sso_ring[idx];
        datara_fast_copy(slot, a, la);
        datara_fast_copy(slot + la, b, lb);
        slot[total] = '\0';
        return slot;
    }
    char* buf = datara_str_scratch_alloc(total);
    if (!buf) return "";
    datara_fast_copy(buf, a, la);
    datara_fast_copy(buf + la, b, lb);
    buf[total] = '\0';
    // P1: the result may become the carrier of a proven-unique append chain
    // (s = s + x in a loop). Stamp ownership into the 16-byte scratch header.
    datara_strbuf_mark_live(buf, (size_t)((total + 1 + 15) & ~15), total);
    return buf;
}

static const char DIGITS_LUT[201] =
    "00010203040506070809"
    "10111213141516171819"
    "20212223242526272829"
    "30313233343536373839"
    "40414243444546474849"
    "50515253545556575859"
    "60616263646566676869"
    "70717273747576777879"
    "80818283848586878889"
    "90919293949596979899";

size_t fast_i64toa(int64_t val, char* buf) {
    if (val >= 0 && val < 10000) {
        if (val < 10) {
            buf[0] = (char)('0' + val);
            buf[1] = '\0';
            return 1;
        }
        if (val < 100) {
            memcpy(buf, &DIGITS_LUT[val * 2], 2);
            buf[2] = '\0';
            return 2;
        }
        uint32_t v = (uint32_t)val;
        uint32_t q = v / 100;
        uint32_t r = v - (q * 100);
        if (q >= 10) {
            memcpy(buf, &DIGITS_LUT[q * 2], 2);
            memcpy(buf + 2, &DIGITS_LUT[r * 2], 2);
            buf[4] = '\0';
            return 4;
        } else {
            buf[0] = (char)('0' + q);
            memcpy(buf + 1, &DIGITS_LUT[r * 2], 2);
            buf[3] = '\0';
            return 3;
        }
    } else if (val >= 0 && val < 1000000) {
        uint32_t v = (uint32_t)val;
        uint32_t q1 = v / 10000;
        uint32_t rem = v - (q1 * 10000);
        uint32_t q2 = rem / 100;
        uint32_t r = rem - (q2 * 100);
        char* p = buf;
        if (q1 >= 10) {
            memcpy(p, &DIGITS_LUT[q1 * 2], 2);
            p += 2;
        } else {
            *p++ = (char)('0' + q1);
        }
        memcpy(p, &DIGITS_LUT[q2 * 2], 2);
        p += 2;
        memcpy(p, &DIGITS_LUT[r * 2], 2);
        p += 2;
        *p = '\0';
        return (size_t)(p - buf);
    }
    char temp[32];
    char* p = temp + 31;
    uint64_t uval;
    if (val < 0) {
        uval = (uint64_t)(-(val + 1)) + 1;
    } else {
        uval = (uint64_t)val;
    }
    while (uval >= 100) {
        uint64_t q = uval / 100;
        uint32_t r = (uint32_t)(uval - (q * 100));
        uval = q;
        p -= 2;
        memcpy(p, &DIGITS_LUT[r * 2], 2);
    }
    if (uval >= 10) {
        p -= 2;
        memcpy(p, &DIGITS_LUT[uval * 2], 2);
    } else {
        *--p = (char)('0' + uval);
    }
    size_t len = (size_t)((temp + 31) - p);
    char* dst = buf;
    if (val < 0) {
        *dst++ = '-';
        len++;
    }
    memcpy(dst, p, (size_t)((temp + 31) - p));
    buf[len] = '\0';
    return len;
}

static DATARA_TLS char tls_int_bufs[256][32];
static DATARA_TLS uint32_t tls_int_idx = 0;

const char* datara_rt_int_to_str(int64_t v) {
    char* buf = tls_int_bufs[tls_int_idx & 255];
    tls_int_idx++;
    size_t len = fast_i64toa(v, buf);
    buf[31] = (char)len;
    return buf;
}

static inline size_t datara_fast_strlen(const char* s) {
    if (!s) return 0;
    ptrdiff_t diff = s - &tls_int_bufs[0][0];
    if ((size_t)diff < sizeof(tls_int_bufs) && (((size_t)diff) & 31) == 0) {
        return (size_t)(uint8_t)s[31];
    }
    return strlen(s);
}

const char* datara_rt_str_concat_3(const char* a, const char* b, const char* c) {
    size_t la = datara_fast_strlen(a);
    size_t lb = datara_fast_strlen(b);
    size_t lc = datara_fast_strlen(c);
    size_t total = la + lb + lc;
    char* buf = datara_str_scratch_alloc(total);
    if (!buf) return "";
    char* p = buf;
    if (la) { datara_fast_copy(p, a, la); p += la; }
    if (lb) { datara_fast_copy(p, b, lb); p += lb; }
    if (lc) { datara_fast_copy(p, c, lc); p += lc; }
    *p = '\0';
    return buf;
}

const char* datara_rt_str_concat_4(const char* a, const char* b, const char* c, const char* d) {
    size_t la = datara_fast_strlen(a);
    size_t lb = datara_fast_strlen(b);
    size_t lc = datara_fast_strlen(c);
    size_t ld = datara_fast_strlen(d);
    size_t total = la + lb + lc + ld;
    char* buf = datara_str_scratch_alloc(total);
    if (!buf) return "";
    char* p = buf;
    if (la) { datara_fast_copy(p, a, la); p += la; }
    if (lb) { datara_fast_copy(p, b, lb); p += lb; }
    if (lc) { datara_fast_copy(p, c, lc); p += lc; }
    if (ld) { datara_fast_copy(p, d, ld); p += ld; }
    *p = '\0';
    return buf;
}

const char* datara_rt_str_concat_5(const char* a, const char* b, const char* c, const char* d, const char* e) {
    size_t la = datara_fast_strlen(a);
    size_t lb = datara_fast_strlen(b);
    size_t lc = datara_fast_strlen(c);
    size_t ld = datara_fast_strlen(d);
    size_t le = datara_fast_strlen(e);
    size_t total = la + lb + lc + ld + le;
    char* buf = datara_str_scratch_alloc(total);
    if (!buf) return "";
    char* p = buf;
    if (la) { datara_fast_copy(p, a, la); p += la; }
    if (lb) { datara_fast_copy(p, b, lb); p += lb; }
    if (lc) { datara_fast_copy(p, c, lc); p += lc; }
    if (ld) { datara_fast_copy(p, d, ld); p += ld; }
    if (le) { datara_fast_copy(p, e, le); p += le; }
    *p = '\0';
    return buf;
}

const char* datara_rt_format_str_i64_str_i64(const char* s1, int64_t n1, const char* s2, int64_t n2) {
    size_t l1 = s1 ? datara_fast_strlen(s1) : 0;
    size_t l2 = s2 ? datara_fast_strlen(s2) : 0;
    size_t max_needed = l1 + l2 + 50;
    char* buf = datara_str_scratch_alloc(max_needed);
    if (!buf) return "";
    char* p = buf;
    if (l1) {
        datara_fast_copy(p, s1, l1);
        p += l1;
    }
    p += fast_i64toa(n1, p);
    if (l2) {
        datara_fast_copy(p, s2, l2);
        p += l2;
    }
    p += fast_i64toa(n2, p);
    *p = '\0';
    return buf;
}

void datara_rt_scratch_reset(void) {
    tls_scratch_offset = 0;
}
