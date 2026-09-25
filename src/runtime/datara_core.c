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

uint32_t datara_rt_abi_version(void) {
    return DATARA_RT_ABI_VERSION;
}

void datara_rt_out_int(int64_t v) {
    datara_rt_print_int(v);
    datara_rt_print_newline();
}

void datara_rt_out_bool(int64_t v) {
    datara_rt_print_bool(v);
    datara_rt_print_newline();
}

// Returns a pointer to a static literal; never freed. The runtime's string
// concat also never frees its inputs, so this is safe in the same way.
const char* datara_rt_bool_to_str(int64_t v) {
    return v ? "true" : "false";
}

void datara_rt_out_float(double v) {
    datara_rt_print_float(v);
    datara_rt_print_newline();
}

static DATARA_TLS char tls_float_bufs[64][64];
static DATARA_TLS uint32_t tls_float_idx = 0;

const char* datara_rt_float_to_str(double v) {
    if (isnan(v)) {
        return signbit(v) ? "-NaN" : "NaN";
    }
    if (isinf(v)) {
        return v < 0 ? "-Infinity" : "Infinity";
    }
    char* buf = tls_float_bufs[(tls_float_idx++) & 63];
    if (v == floor(v) && fabs(v) < 1e15) {
        snprintf(buf, 64, "%.0f", v);
        return buf;
    }
    for (int prec = 4; prec < 17; prec++) {
        snprintf(buf, 64, "%.*g", prec, v);
        if (strtod(buf, NULL) == v) {
            return buf;
        }
    }
    snprintf(buf, 64, "%.17g", v);
    return buf;
}

void datara_rt_out_str(const char* s) {
    if (!s) {
        datara_rt_panic("invariant violation: null string pointer passed to 'out' statement");
    }
    datara_rt_print_str(s);
    datara_rt_print_newline();
}

void datara_rt_err(const char* s) {
    if (!s) {
        datara_rt_panic("invariant violation: null string pointer passed to 'err' statement");
    }
    fprintf(stderr, "%s\n", s);
}

void datara_rt_exit(int32_t code) {
    datara_rt_pgo_auto_flush();
    exit(code);
}

#define DATARA_SCRATCH_RING_SIZE (1024 * 1024)

static int g_argc = 0;
static char** g_argv = NULL;

void datara_rt_set_args(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
}

int64_t datara_rt_args_count(void) {
    return (int64_t)g_argc;
}

const char* datara_rt_args_get(int64_t idx) {
    if (idx < 0 || idx >= g_argc || !g_argv) return "";
    return g_argv[idx];
}
// ============================================================================
// Phase 17: Runtime Systems Layer
// ============================================================================

// 1. Chase-Lev Work-Stealing Deque (Lock-Free SPMC)
struct DataraChaseLevDeque {
    volatile int64_t top;
    volatile int64_t bottom;
    int64_t* volatile buffer;
    volatile int64_t capacity;
    volatile int64_t mask;
    volatile uint64_t resize_seq;
    int64_t** retired_buffers;
    size_t retired_count;
    size_t retired_capacity;
};

DataraChaseLevDeque* datara_rt_chase_lev_create(int64_t capacity) {
    if (capacity <= 0) capacity = 1024;
    int64_t cap = 1;
    while (cap < capacity) cap <<= 1;

    DataraChaseLevDeque* q = (DataraChaseLevDeque*)malloc(sizeof(DataraChaseLevDeque));
    if (!q) return NULL;
    q->top = 0;
    q->bottom = 0;
    q->capacity = cap;
    q->mask = cap - 1;
    q->resize_seq = 0;
    q->retired_buffers = NULL;
    q->retired_count = 0;
    q->retired_capacity = 0;
    q->buffer = (int64_t*)malloc(sizeof(int64_t) * (size_t)cap);
    if (!q->buffer) {
        free(q);
        return NULL;
    }
    return q;
}

void datara_rt_chase_lev_destroy(DataraChaseLevDeque* q) {
    if (q) {
        if (q->buffer) {
            free(q->buffer);
            q->buffer = NULL;
        }
        if (q->retired_buffers) {
            for (size_t i = 0; i < q->retired_count; i++) {
                if (q->retired_buffers[i]) {
                    free(q->retired_buffers[i]);
                }
            }
            free(q->retired_buffers);
            q->retired_buffers = NULL;
        }
        free(q);
    }
}

void datara_rt_chase_lev_push(DataraChaseLevDeque* q, int64_t task_id) {
    if (!q || !q->buffer) return;
    int64_t b = q->bottom;
    int64_t t = q->top;
    if (b - t >= q->capacity) {
        int64_t old_cap = q->capacity;
        int64_t new_cap = old_cap * 2;
        int64_t* new_buf = (int64_t*)malloc(sizeof(int64_t) * (size_t)new_cap);
        if (new_buf) {
#ifdef _WIN32
            InterlockedIncrement64((volatile LONG64*)&q->resize_seq);
            MemoryBarrier();
#else
            __sync_add_and_fetch(&q->resize_seq, 1);
            __sync_synchronize();
#endif
            t = q->top;
            for (int64_t i = t; i < b; i++) {
                new_buf[i & (new_cap - 1)] = q->buffer[i & q->mask];
            }
            if (q->retired_count >= q->retired_capacity) {
                size_t new_rcap = q->retired_capacity ? q->retired_capacity * 2 : 16;
                int64_t** new_retired = (int64_t**)realloc(q->retired_buffers, sizeof(int64_t*) * new_rcap);
                if (new_retired) {
                    q->retired_buffers = new_retired;
                    q->retired_capacity = new_rcap;
                }
            }
            if (q->retired_count < q->retired_capacity) {
                q->retired_buffers[q->retired_count++] = q->buffer;
            }
            q->mask = new_cap - 1;
            q->capacity = new_cap;
            q->buffer = new_buf;
#ifdef _WIN32
            MemoryBarrier();
            InterlockedIncrement64((volatile LONG64*)&q->resize_seq);
            MemoryBarrier();
#else
            __sync_synchronize();
            __sync_add_and_fetch(&q->resize_seq, 1);
            __sync_synchronize();
#endif
        }
    }
    q->buffer[b & q->mask] = task_id;
#ifdef _WIN32
    MemoryBarrier();
#else
    __sync_synchronize();
#endif
    q->bottom = b + 1;
}

int64_t datara_rt_chase_lev_pop(DataraChaseLevDeque* q) {
    if (!q || !q->buffer) return -1;
    int64_t b = q->bottom - 1;
    q->bottom = b;
#ifdef _WIN32
    MemoryBarrier();
#else
    __sync_synchronize();
#endif
    int64_t t = q->top;
    if (t <= b) {
        int64_t val = q->buffer[b & q->mask];
        if (t == b) {
#ifdef _WIN32
            if (InterlockedCompareExchange64(&q->top, t + 1, t) != t) {
                val = -1;
            }
#else
            if (!__sync_bool_compare_and_swap(&q->top, t, t + 1)) {
                val = -1;
            }
#endif
            q->bottom = t + 1;
        }
        return val;
    } else {
        q->bottom = t;
        return -1;
    }
}

int64_t datara_rt_chase_lev_steal(DataraChaseLevDeque* q) {
    if (!q || !q->buffer) return -1;
    while (1) {
        uint64_t seq1 = q->resize_seq;
        if (seq1 & 1) {
#ifdef _WIN32
            SwitchToThread();
#else
            sched_yield();
#endif
            continue;
        }
        int64_t t = q->top;
#ifdef _WIN32
        MemoryBarrier();
#else
        __sync_synchronize();
#endif
        int64_t b = q->bottom;
        if (t >= b) {
            return -1; // Empty
        }
#ifdef _WIN32
        MemoryBarrier();
#else
        __sync_synchronize();
#endif
        int64_t* buf = q->buffer;
        int64_t mask = q->mask;
        int64_t val = buf[t & mask];
#ifdef _WIN32
        MemoryBarrier();
#else
        __sync_synchronize();
#endif
        uint64_t seq2 = q->resize_seq;
        if (seq1 != seq2 || (seq2 & 1)) {
            continue;
        }

#ifdef _WIN32
        if (InterlockedCompareExchange64(&q->top, t + 1, t) == t) {
            return val;
        }
#else
        if (__sync_bool_compare_and_swap(&q->top, t, t + 1)) {
            return val;
        }
#endif
        // Lost race to another thief or owner pop: retry
    }
}

int64_t datara_rt_chase_lev_size(DataraChaseLevDeque* q) {
    if (!q) return 0;
    int64_t b = q->bottom;
    int64_t t = q->top;
    return b >= t ? (b - t) : 0;
}

// 2. SIMD-Accelerated Fast Memory Operations
void* datara_rt_fast_memcpy(void* dest, const void* src, size_t n) {
    if (!dest || !src || n == 0) return dest;
    uint8_t* d = (uint8_t*)dest;
    const uint8_t* s = (const uint8_t*)src;

    // 64-byte unrolled loop (8 x 64-bit uint64 words)
    while (n >= 64) {
        uint64_t w0, w1, w2, w3, w4, w5, w6, w7;
        memcpy(&w0, s + 0, 8);
        memcpy(&w1, s + 8, 8);
        memcpy(&w2, s + 16, 8);
        memcpy(&w3, s + 24, 8);
        memcpy(&w4, s + 32, 8);
        memcpy(&w5, s + 40, 8);
        memcpy(&w6, s + 48, 8);
        memcpy(&w7, s + 56, 8);

        memcpy(d + 0, &w0, 8);
        memcpy(d + 8, &w1, 8);
        memcpy(d + 16, &w2, 8);
        memcpy(d + 24, &w3, 8);
        memcpy(d + 32, &w4, 8);
        memcpy(d + 40, &w5, 8);
        memcpy(d + 48, &w6, 8);
        memcpy(d + 56, &w7, 8);

        d += 64;
        s += 64;
        n -= 64;
    }

    // 8-byte chunks
    while (n >= 8) {
        uint64_t w;
        memcpy(&w, s, 8);
        memcpy(d, &w, 8);
        d += 8;
        s += 8;
        n -= 8;
    }

    // Residual bytes
    while (n > 0) {
        *d++ = *s++;
        n--;
    }
    return dest;
}

void* datara_rt_fast_memset(void* dest, int c, size_t n) {
    if (!dest || n == 0) return dest;
    uint8_t* d = (uint8_t*)dest;
    uint8_t b_val = (uint8_t)c;
    uint64_t w = 0x0101010101010101ULL * (uint64_t)b_val;

    while (n >= 64) {
        memcpy(d + 0, &w, 8);
        memcpy(d + 8, &w, 8);
        memcpy(d + 16, &w, 8);
        memcpy(d + 24, &w, 8);
        memcpy(d + 32, &w, 8);
        memcpy(d + 40, &w, 8);
        memcpy(d + 48, &w, 8);
        memcpy(d + 56, &w, 8);
        d += 64;
        n -= 64;
    }

    while (n >= 8) {
        memcpy(d, &w, 8);
        d += 8;
        n -= 8;
    }

    while (n > 0) {
        *d++ = b_val;
        n--;
    }
    return dest;
}

int datara_rt_fast_strncmp(const char* s1, const char* s2, size_t n) {
    if (n == 0 || !s1 || !s2) return 0;
    const uint8_t* p1 = (const uint8_t*)s1;
    const uint8_t* p2 = (const uint8_t*)s2;

    while (n >= 8) {
        uint64_t v1, v2;
        memcpy(&v1, p1, 8);
        memcpy(&v2, p2, 8);

        // Check for null terminator in v1
        uint64_t has_zero = (v1 - 0x0101010101010101ULL) & ~v1 & 0x8080808080808080ULL;
        if (v1 != v2 || has_zero) {
            for (int i = 0; i < 8; i++) {
                if (p1[i] != p2[i] || p1[i] == 0) {
                    return (int)p1[i] - (int)p2[i];
                }
            }
        }
        p1 += 8;
        p2 += 8;
        n -= 8;
    }

    while (n > 0) {
        if (*p1 != *p2 || *p1 == 0) {
            return (int)*p1 - (int)*p2;
        }
        p1++;
        p2++;
        n--;
    }
    return 0;
}

int datara_rt_fast_memcmp(const void* s1, const void* s2, size_t n) {
    if (n == 0 || !s1 || !s2) return 0;
    const uint8_t* p1 = (const uint8_t*)s1;
    const uint8_t* p2 = (const uint8_t*)s2;

    while (n >= 8) {
        uint64_t v1, v2;
        memcpy(&v1, p1, 8);
        memcpy(&v2, p2, 8);
        if (v1 != v2) {
            for (int i = 0; i < 8; i++) {
                if (p1[i] != p2[i]) {
                    return (int)p1[i] - (int)p2[i];
                }
            }
        }
        p1 += 8;
        p2 += 8;
        n -= 8;
    }

    while (n > 0) {
        if (*p1 != *p2) {
            return (int)*p1 - (int)*p2;
        }
        p1++;
        p2++;
        n--;
    }
    return 0;
}

// 3. Thread Pinning / Core Affinity
int datara_rt_pin_thread(int64_t core_id) {
    if (core_id < 0) return -1;
#ifdef _WIN32
    DWORD_PTR mask = (DWORD_PTR)1 << (core_id % 64);
    DWORD_PTR prev = SetThreadAffinityMask(GetCurrentThread(), mask);
    return prev != 0 ? 0 : -1;
#elif defined(__APPLE__)
    (void)core_id;
    return 0;
#else
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    CPU_SET(core_id, &cpuset);
    return pthread_setaffinity_np(pthread_self(), sizeof(cpu_set_t), &cpuset);
#endif
}

int64_t datara_rt_get_current_core(void) {
#ifdef _WIN32
    return (int64_t)GetCurrentProcessorNumber();
#elif defined(__APPLE__)
    return 0;
#else
    return (int64_t)sched_getcpu();
#endif
}

// ============================================================================
// Capability Lattice: Hardware/Runtime Capability Traps
// ============================================================================
#if defined(_WIN32)
static volatile LONG64 g_datara_active_caps = (LONG64)DATARA_CAP_ALL;
#else
static volatile uint64_t g_datara_active_caps = DATARA_CAP_ALL;
#endif
static volatile int g_datara_caps_initialized = 0;

static void datara_rt_cap_init_if_needed(void) {
    if (g_datara_caps_initialized) return;
    g_datara_caps_initialized = 1;
    const char* env_mask = getenv("DATARA_CAP_MASK");
    if (env_mask && env_mask[0]) {
        uint64_t mask = (uint64_t)strtoull(env_mask, NULL, 0);
        datara_rt_cap_set_mask(mask);
    } else {
        const char* sandbox = getenv("DATARA_SANDBOX");
        if (sandbox && (strcmp(sandbox, "1") == 0 || strcmp(sandbox, "true") == 0)) {
            datara_rt_cap_set_mask(DATARA_CAP_FS_READ | DATARA_CAP_NET_CLIENT);
        }
    }
}

void datara_rt_cap_set_mask(uint64_t mask) {
    g_datara_caps_initialized = 1;
#if defined(_WIN32)
    InterlockedExchange64(&g_datara_active_caps, (LONG64)mask);
#elif defined(__GNUC__) || defined(__clang__)
    __atomic_store_n(&g_datara_active_caps, mask, __ATOMIC_SEQ_CST);
#else
    g_datara_active_caps = mask;
#endif
}

uint64_t datara_rt_cap_get_mask(void) {
    datara_rt_cap_init_if_needed();
#if defined(_WIN32)
    return (uint64_t)InterlockedCompareExchange64(&g_datara_active_caps, 0, 0);
#elif defined(__GNUC__) || defined(__clang__)
    return __atomic_load_n(&g_datara_active_caps, __ATOMIC_SEQ_CST);
#else
    return g_datara_active_caps;
#endif
}

void datara_rt_cap_revoke(uint64_t mask) {
#if defined(_WIN32)
    InterlockedAnd64(&g_datara_active_caps, (LONG64)~mask);
#elif defined(__GNUC__) || defined(__clang__)
    __atomic_and_fetch(&g_datara_active_caps, ~mask, __ATOMIC_SEQ_CST);
#else
    g_datara_active_caps &= ~mask;
#endif
}

void datara_rt_cap_grant(uint64_t mask) {
#if defined(_WIN32)
    InterlockedOr64(&g_datara_active_caps, (LONG64)mask);
#elif defined(__GNUC__) || defined(__clang__)
    __atomic_or_fetch(&g_datara_active_caps, mask, __ATOMIC_SEQ_CST);
#else
    g_datara_active_caps |= mask;
#endif
}

void datara_rt_trigger_hardware_cap_trap(uint64_t required_bit, const char* op_name) {
    uint64_t current_mask = datara_rt_cap_get_mask();
    datara_rt_flush();
    fprintf(stderr,
        "[DATARA HARDWARE CAPABILITY TRAP] Security violation: operation '%s' requires capability 0x%llx (active mask: 0x%llx). Aborting execution.\n",
        op_name ? op_name : "unknown",
        (unsigned long long)required_bit,
        (unsigned long long)current_mask);
    datara_rt_print_backtrace();
    fflush(stderr);

    // Hardware trap execution:
#if defined(__GNUC__) || defined(__clang__)
    #if defined(__x86_64__) || defined(_M_X64)
        __asm__ volatile("ud2");
    #else
        __builtin_trap();
    #endif
#elif defined(_MSC_VER)
    #if defined(_M_X64) || defined(_M_IX86)
        __debugbreak();
    #endif
#endif
    exit(132); // Fallback exit code
}

void datara_rt_cap_require(uint64_t required_bit, const char* op_name) {
    uint64_t current = datara_rt_cap_get_mask();
    if ((current & required_bit) != required_bit) {
        datara_rt_trigger_hardware_cap_trap(required_bit, op_name);
    }
}


// ---------------------------------------------------------------------------
// v1.4.0 StrBuf: growable string builder with amortized doubling.
// join() materializes through the never-freed scratch ring, so the result
// safely outlives further builder mutation (same ownership conventions as
// every other runtime string).
// ---------------------------------------------------------------------------
typedef struct DataraStrBuf {
    size_t len;
    size_t cap;
    char* data;
} DataraStrBuf;

void* datara_rt_strbuf_new(void) {
    DataraStrBuf* sb = (DataraStrBuf*)malloc(sizeof(DataraStrBuf));
    if (!sb) return NULL;
    sb->cap = 64;
    sb->len = 0;
    sb->data = (char*)malloc(sb->cap);
    if (!sb->data) {
        free(sb);
        return NULL;
    }
    sb->data[0] = '\0';
    return sb;
}

static int datara_strbuf_reserve(DataraStrBuf* sb, size_t extra) {
    size_t needed = sb->len + extra + 1;
    if (needed <= sb->cap) return 1;
    size_t new_cap = sb->cap;
    while (new_cap < needed) {
        new_cap *= 2;
    }
    char* grown = (char*)realloc(sb->data, new_cap);
    if (!grown) return 0;
    sb->data = grown;
    sb->cap = new_cap;
    return 1;
}

void* datara_rt_strbuf_push(void* sb_ptr, const char* s) {
    DataraStrBuf* sb = (DataraStrBuf*)sb_ptr;
    if (!sb) return NULL;
    if (!s || s[0] == '\0') return sb;
    size_t slen = strlen(s);
    if (!datara_strbuf_reserve(sb, slen)) return sb;
    datara_fast_copy(sb->data + sb->len, s, slen);
    sb->len += slen;
    sb->data[sb->len] = '\0';
    return sb;
}

void* datara_rt_strbuf_push_int(void* sb_ptr, int64_t v) {
    DataraStrBuf* sb = (DataraStrBuf*)sb_ptr;
    if (!sb) return NULL;
    char tmp[32];
    snprintf(tmp, sizeof(tmp), "%lld", (long long)v);
    return datara_rt_strbuf_push(sb, tmp);
}

const char* datara_rt_strbuf_join(void* sb_ptr) {
    DataraStrBuf* sb = (DataraStrBuf*)sb_ptr;
    if (!sb) return "";
    char* out = datara_str_scratch_alloc(sb->len + 1);
    if (!out) return "";
    datara_fast_copy(out, sb->data, sb->len);
    out[sb->len] = '\0';
    return out;
}

int64_t datara_rt_strbuf_len(void* sb_ptr) {
    DataraStrBuf* sb = (DataraStrBuf*)sb_ptr;
    if (!sb) return 0;
    return (int64_t)sb->len;
}

void datara_rt_strbuf_free(void* sb_ptr) {
    DataraStrBuf* sb = (DataraStrBuf*)sb_ptr;
    if (!sb) return;
    free(sb->data);
    free(sb);
}

// ============================================================================
// 4-Tier Memory & Direct Hardware Spectrum Implementation
// ============================================================================
int64_t datara_rt_arena_used(void) {
    return datara_rt_arena_checkpoint();
}

void datara_rt_arena_clear(void) {
    datara_rt_arena_reset(0);
}

void* datara_rt_mem_alloc(int64_t bytes) {
    if (bytes <= 0) return NULL;
    return malloc((size_t)bytes);
}

void datara_rt_mem_free(void* ptr) {
    if (ptr) free(ptr);
}

void datara_rt_mem_copy(void* dst, const void* src, int64_t bytes) {
    if (!dst || !src || bytes <= 0) return;
    datara_fast_copy(dst, src, (size_t)bytes);
}

int64_t datara_rt_ptr_read_i64(const void* ptr, int64_t offset) {
    if (!ptr) return 0;
    const char* byte_ptr = (const char*)ptr + offset;
    int64_t val = 0;
    memcpy(&val, byte_ptr, sizeof(int64_t));
    return val;
}

void datara_rt_ptr_write_i64(void* ptr, int64_t offset, int64_t val) {
    if (!ptr) return;
    char* byte_ptr = (char*)ptr + offset;
    memcpy(byte_ptr, &val, sizeof(int64_t));
}

double datara_rt_ptr_read_f64(const void* ptr, int64_t offset) {
    if (!ptr) return 0.0;
    const char* byte_ptr = (const char*)ptr + offset;
    double val = 0.0;
    memcpy(&val, byte_ptr, sizeof(double));
    return val;
}

void datara_rt_ptr_write_f64(void* ptr, int64_t offset, double val) {
    if (!ptr) return;
    char* byte_ptr = (char*)ptr + offset;
    memcpy(byte_ptr, &val, sizeof(double));
}

int64_t datara_rt_ptr_read_u8(const void* ptr, int64_t offset) {
    if (!ptr) return 0;
    const uint8_t* byte_ptr = (const uint8_t*)ptr + offset;
    return (int64_t)(*byte_ptr);
}

void datara_rt_ptr_write_u8(void* ptr, int64_t offset, int64_t val) {
    if (!ptr) return;
    uint8_t* byte_ptr = (uint8_t*)ptr + offset;
    *byte_ptr = (uint8_t)(val & 0xFF);
}

void datara_rt_cpu_fence(void) {
#ifdef _MSC_VER
    MemoryBarrier();
#else
    __sync_synchronize();
#endif
}

void datara_rt_cpu_prefetch(const void* ptr) {
    if (!ptr) return;
#if defined(__GNUC__) || defined(__clang__)
    __builtin_prefetch(ptr, 0, 3);
#elif defined(_MSC_VER) && (defined(_M_X64) || defined(_M_IX86))
    volatile char dummy = *(const volatile char*)ptr;
    (void)dummy;
#endif
}

// Module-level global state storage
typedef struct {
    char name[64];
    int64_t val;
} DataraModuleGlobal;

static DataraModuleGlobal g_datara_module_globals[512];
static size_t g_datara_module_global_count = 0;

void datara_rt_global_set(const char* name, int64_t val) {
    if (!name) return;
    for (size_t i = 0; i < g_datara_module_global_count; ++i) {
        if (strcmp(g_datara_module_globals[i].name, name) == 0) {
            g_datara_module_globals[i].val = val;
            return;
        }
    }
    if (g_datara_module_global_count < 512) {
#if defined(_MSC_VER)
        strncpy_s(g_datara_module_globals[g_datara_module_global_count].name, 64, name, 63);
#else
        strncpy(g_datara_module_globals[g_datara_module_global_count].name, name, 63);
#endif
        g_datara_module_globals[g_datara_module_global_count].name[63] = '\0';
        g_datara_module_globals[g_datara_module_global_count].val = val;
        g_datara_module_global_count++;
    }
}

int64_t datara_rt_global_get(const char* name) {
    if (!name) return 0;
    for (size_t i = 0; i < g_datara_module_global_count; ++i) {
        if (strcmp(g_datara_module_globals[i].name, name) == 0) {
            return g_datara_module_globals[i].val;
        }
    }
    return 0;
}

void* datara_rt_stack_alloc(int64_t bytes) { return datara_rt_arena_alloc(bytes); }
void* stack_alloc(int64_t bytes) { return datara_rt_stack_alloc(bytes); }

int64_t* datara_rt_list_append_slow(int64_t* list, int64_t val) {
    return datara_rt_list_append(list, val);
}

// Aliases without datara_rt_ prefix for direct C ABI invocations
void* arena_alloc(int64_t bytes) { return datara_rt_arena_alloc(bytes); }
void arena_reset(int64_t saved_top) { datara_rt_arena_reset(saved_top); }
int64_t arena_used(void) { return datara_rt_arena_used(); }
void arena_clear(void) { datara_rt_arena_clear(); }
void* mem_alloc(int64_t bytes) { return datara_rt_mem_alloc(bytes); }
void mem_free(void* ptr) { datara_rt_mem_free(ptr); }
void mem_copy(void* dest, const void* src, int64_t bytes) { datara_rt_mem_copy(dest, src, bytes); }
int64_t ptr_read_i64(const void* ptr, int64_t offset_bytes) { return datara_rt_ptr_read_i64(ptr, offset_bytes); }
void ptr_write_i64(void* ptr, int64_t offset_bytes, int64_t val) { datara_rt_ptr_write_i64(ptr, offset_bytes, val); }
double ptr_read_f64(const void* ptr, int64_t offset_bytes) { return datara_rt_ptr_read_f64(ptr, offset_bytes); }
void ptr_write_f64(void* ptr, int64_t offset_bytes, double val) { datara_rt_ptr_write_f64(ptr, offset_bytes, val); }
int64_t ptr_read_u8(const void* ptr, int64_t offset_bytes) { return datara_rt_ptr_read_u8(ptr, offset_bytes); }
void ptr_write_u8(void* ptr, int64_t offset_bytes, int64_t val) { datara_rt_ptr_write_u8(ptr, offset_bytes, val); }
void cpu_fence(void) { datara_rt_cpu_fence(); }
void cpu_prefetch(const void* ptr) { datara_rt_cpu_prefetch(ptr); }
