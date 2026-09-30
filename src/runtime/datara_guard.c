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
// ============================================================================
// Graduated Ownership Runtime Guards
// Thread-unsafe by default — guards are per-thread
// ============================================================================
typedef struct {
    int64_t val;
    uint32_t refcount;
    uint32_t active;
} DataraOwnGuardSlot;

#define DATARA_OWN_MAX_GUARDS 512
static DATARA_TLS DataraOwnGuardSlot g_datara_own_slots[DATARA_OWN_MAX_GUARDS];
static DATARA_TLS uint32_t g_datara_own_next_slot = 1;

int64_t datara_rt_own_acquire(int64_t val) {
    for (uint32_t i = 1; i < g_datara_own_next_slot && i < DATARA_OWN_MAX_GUARDS; i++) {
        if (g_datara_own_slots[i].active && g_datara_own_slots[i].val == val) {
            g_datara_own_slots[i].refcount++;
            return val;
        }
    }
    uint32_t slot_idx = 0;
    for (uint32_t i = 1; i < g_datara_own_next_slot && i < DATARA_OWN_MAX_GUARDS; i++) {
        if (!g_datara_own_slots[i].active) {
            slot_idx = i;
            break;
        }
    }
    if (slot_idx == 0 && g_datara_own_next_slot < DATARA_OWN_MAX_GUARDS) {
        slot_idx = g_datara_own_next_slot++;
    }
    if (slot_idx > 0 && slot_idx < DATARA_OWN_MAX_GUARDS) {
        g_datara_own_slots[slot_idx].val = val;
        g_datara_own_slots[slot_idx].refcount = 1;
        g_datara_own_slots[slot_idx].active = 1;
    }
    return val;
}

void datara_rt_own_release(int64_t val) {
    for (uint32_t i = 1; i < g_datara_own_next_slot && i < DATARA_OWN_MAX_GUARDS; i++) {
        if (g_datara_own_slots[i].active && g_datara_own_slots[i].val == val) {
            if (g_datara_own_slots[i].refcount > 0) {
                g_datara_own_slots[i].refcount--;
            }
            if (g_datara_own_slots[i].refcount == 0) {
                g_datara_own_slots[i].active = 0;
                g_datara_own_slots[i].val = 0;
            }
            return;
        }
    }
}

// ============================================================================
// M5 TMM Layer 2: true ARC (atomic reference counting) primitives.
// The object layout carries the refcount at a NEGATIVE offset from the
// user pointer (-16), so datara objects stay passable to existing C code
// that reads field offsets 0..n directly. Header word 0 holds the strong
// count, header word 1 the live weak-handle count (weak never keeps the
// object alive, but the header must outlive weak handles so upgrade()
// can answer "dead" instead of crashing).
// Release is deterministic: at strong==0 the object is freed. No
// stop-the-world pass, no tracing — matches PLAN_V150 TMM semantics.
//
// ARC header layout (16 bytes, placed before the returned pointer):
//   [-16] strong refcount (atomic int64)
//   [-8]  weak count (atomic int64): number of live Weak handles; the
//         allocation dies only when strong==0 AND weak==0. Weak upgrade
//         CAS-retries against the strong count so check+acquire is atomic.
// ============================================================================

#ifdef _WIN32
  #define DATARA_ATOMIC_CAS64(ptr, oldv, newv) \
      (InterlockedCompareExchange64((LONG64 volatile*)(ptr), (LONG64)(newv), (LONG64)(oldv)))
  #define DATARA_ATOMIC_INC64(ptr) InterlockedIncrement64((LONG64 volatile*)(ptr))
  #define DATARA_ATOMIC_DEC64(ptr) InterlockedDecrement64((LONG64 volatile*)(ptr))
#else
  #define DATARA_ATOMIC_CAS64(ptr, oldv, newv) \
      (__sync_val_compare_and_swap((ptr), (oldv), (newv)))
  #define DATARA_ATOMIC_INC64(ptr) __sync_add_and_fetch((ptr), 1)
  #define DATARA_ATOMIC_DEC64(ptr) __sync_sub_and_fetch((ptr), 1)
#endif

#define DATARA_ARC_HEADER_SLOTS 2

static int64_t* datara_arc_header_of(void* obj) {
    return ((int64_t*)obj) - DATARA_ARC_HEADER_SLOTS;
}

// First acquire of a freshly allocated @arc object: sets strong=1.
// The allocation must include the 16-byte ARC header (the codegen alloc
// path adds DATARA_ARC_HEADER_SLOTS*8 to the requested byte size).
int64_t datara_rt_arc_init(void* obj) {
    if (!obj) return 0;
    int64_t* hdr = datara_arc_header_of(obj);
    hdr[0] = 1; // strong
    hdr[1] = 0; // weak
    return (int64_t)(uintptr_t)obj;
}

int64_t datara_rt_arc_acquire(void* obj) {
    if (!obj) return 0;
    int64_t* hdr = datara_arc_header_of(obj);
    int64_t prev = hdr[0];
    for (;;) {
        if (prev <= 0) {
            // Object already dead: acquire of a dying object is a bug the
            // static checker prevents; fail loudly instead of resurrecting.
            datara_rt_arc_use_after_free(obj);
        }
        int64_t want = prev + 1;
        int64_t observed = DATARA_ATOMIC_CAS64(hdr, prev, want);
        if (observed == prev) return (int64_t)(uintptr_t)obj;
        prev = observed;
    }
}

void datara_rt_arc_release(void* obj) {
    if (!obj) return;
    int64_t* hdr = datara_arc_header_of(obj);
    int64_t prev = hdr[0];
    for (;;) {
        if (prev <= 0) {
            datara_rt_arc_use_after_free(obj);
        }
        int64_t want = prev - 1;
        int64_t observed = DATARA_ATOMIC_CAS64(hdr, prev, want);
        if (observed != prev) { prev = observed; continue; }
        if (want == 0 && hdr[1] == 0) {
            // Last strong reference released and no weak handles keep the
            // header alive: reclaim the whole allocation (header + object).
            datara_rt_free((void*)hdr);
        }
        return;
    }
}

int64_t datara_rt_arc_count(void* obj) {
    if (!obj) return 0;
    return datara_arc_header_of(obj)[0];
}

// Weak handle: a heap cell holding {obj}. Weak creation increments the
// weak count of the target; the handle is freed by weak_drop.
void* datara_rt_weak_create(void* obj) {
    if (!obj) return NULL;
    int64_t* hdr = datara_arc_header_of(obj);
    DATARA_ATOMIC_INC64(&hdr[1]);
    int64_t* cell = (int64_t*)malloc(2 * sizeof(int64_t));
    if (!cell) { DATARA_ATOMIC_DEC64(&hdr[1]); return NULL; }
    cell[0] = (int64_t)(uintptr_t)obj;
    cell[1] = 1; // magic guard for corruption detection
    return (void*)cell;
}

// upgrade(): Outcome<T> — ok(obj) when the target is alive, err("dead")
// when it has been reclaimed. The check+acquire must be atomic, so the
// outcome carries a REAL acquire: callers use the payload and release it
// (compiler-inserted) when done, or drop it via outcome-free convention.
void* datara_rt_weak_upgrade(void* weak_cell) {
    if (!weak_cell) return datara_rt_outcome_build_raw(0, 0, "weak: null");
    int64_t* cell = (int64_t*)weak_cell;
    if (cell[1] != 1) return datara_rt_outcome_build_raw(0, 0, "weak: invalid");
    void* obj = (void*)(uintptr_t)cell[0];
    if (!obj) return datara_rt_outcome_build_raw(0, 0, "weak: dead");
    int64_t* hdr = datara_arc_header_of(obj);
    int64_t prev = hdr[0];
    for (;;) {
        if (prev <= 0) return datara_rt_outcome_build_raw(0, 0, "weak: dead");
        int64_t want = prev + 1;
        int64_t observed = DATARA_ATOMIC_CAS64(hdr, prev, want);
        if (observed == prev) {
            return datara_rt_outcome_build_raw(1, (int64_t)(uintptr_t)obj, "");
        }
        prev = observed;
    }
}

void datara_rt_weak_drop(void* weak_cell) {
    if (!weak_cell) return;
    int64_t* cell = (int64_t*)weak_cell;
    void* obj = (void*)(uintptr_t)cell[0];
    cell[0] = 0;
    cell[1] = 0;
    free(cell);
    if (obj) {
        int64_t* hdr = datara_arc_header_of(obj);
        if (DATARA_ATOMIC_DEC64(&hdr[1]) == 0 && hdr[0] == 0) {
            datara_rt_free((void*)hdr);
        }
    }
}

// Fail-loud path for acquire/release on a dead object. The static checker
// makes this unreachable in verified programs; hitting it at runtime means
// a soundness bug and must trap, not corrupt the heap.
void datara_rt_arc_use_after_free(void* obj) {
    (void)obj;
    fprintf(stderr, "fatal: @arc object used after release (TMM soundness violation)\n");
    abort();
}

