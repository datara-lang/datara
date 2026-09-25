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
int64_t datara_rt_list_get(int64_t* list, int64_t idx) {
    if (!list) return 0;
    int64_t count = list[0];
    if (idx < 0 || idx >= count) return 0;
    return list[idx + 1];
}

int64_t datara_rt_list_get_unchecked(int64_t* list, int64_t idx) {
    return list[idx + 1];
}

int64_t datara_rt_list_len(int64_t* list) {
    return list ? list[0] : 0;
}

int64_t* datara_rt_list_set(int64_t* list, int64_t idx, int64_t v) {
    if (!list) return NULL;
    int64_t count = list[0];
    if (idx < 0 || idx >= count) return list;
    list[idx + 1] = v;
    return list;
}

int64_t* datara_rt_list_set_unchecked(int64_t* list, int64_t idx, int64_t v) {
    list[idx + 1] = v;
    return list;
}

int64_t* datara_rt_list_init_stack(void* stack_buf, int64_t cap) {
    if (!stack_buf || cap <= 0) return NULL;
    DataraListHeader* hdr = (DataraListHeader*)stack_buf;
    hdr->capacity = cap;
    hdr->magic = DATARA_LIST_MAGIC | DATARA_LIST_FLAG_STACK | (cap <= 8 ? DATARA_LIST_FLAG_SMALL : 0);
    int64_t* list = (int64_t*)(hdr + 1);
    list[0] = 0;
    return list;
}

int64_t datara_rt_list_is_small_vec(int64_t* list) {
    if (!list) return 0;
    DataraListHeader* hdr = ((DataraListHeader*)list) - 1;
    if ((hdr->magic & DATARA_LIST_MAGIC_MASK) == DATARA_LIST_MAGIC) {
        return (hdr->magic & DATARA_LIST_FLAG_SMALL) ? 1 : (hdr->capacity <= 8 ? 1 : 0);
    }
    return 0;
}

int64_t* datara_rt_list_create_capacity(int64_t cap) {
    if (cap < 8) cap = 8;
    if (cap > (int64_t)((SIZE_MAX - sizeof(DataraListHeader)) / sizeof(int64_t) - 1)) return NULL;
    size_t alloc_sz = sizeof(DataraListHeader) + (size_t)(cap + 1) * sizeof(int64_t);
    DataraListHeader* hdr = NULL;
    int64_t flag = DATARA_LIST_FLAG_HEAP;
    if (alloc_sz <= 1024) {
        hdr = (DataraListHeader*)datara_rt_pool_alloc(alloc_sz);
        flag = DATARA_LIST_FLAG_POOL;
    } else {
        tls_heap_alloc_count++;
        hdr = (DataraListHeader*)malloc(alloc_sz);
    }
    if (!hdr) return NULL;
    if (cap <= 8) {
        flag |= DATARA_LIST_FLAG_SMALL;
    }
    hdr->capacity = cap;
    hdr->magic = DATARA_LIST_MAGIC | flag;
    int64_t* list = (int64_t*)(hdr + 1);
    list[0] = 0;
    return list;
}

int64_t* datara_rt_list_create(int64_t cap) {
    return datara_rt_list_create_capacity(cap);
}

int64_t* datara_rt_list_append(int64_t* list, int64_t v) {
    if (!list) {
        int64_t* arr = datara_rt_list_create_capacity(8);
        if (!arr) return NULL;
        arr[0] = 1;
        arr[1] = v;
        return arr;
    }
    int64_t count = list[0];
    if (count < 0) return list;
    DataraListHeader* hdr = ((DataraListHeader*)list) - 1;
    if ((hdr->magic & DATARA_LIST_MAGIC_MASK) == DATARA_LIST_MAGIC && hdr->capacity >= count) {
        if (count + 1 > hdr->capacity) {
            if (hdr->capacity > (int64_t)((SIZE_MAX - sizeof(DataraListHeader)) / (2 * sizeof(int64_t)) - 1)) return list;
            int64_t new_cap = hdr->capacity * 2;
            if (new_cap < 8) new_cap = 8;
            size_t new_alloc_sz = sizeof(DataraListHeader) + (size_t)(new_cap + 1) * sizeof(int64_t);
            int64_t flag = hdr->magic & 0xFULL;
            DataraListHeader* new_hdr = NULL;
            if (flag & DATARA_LIST_FLAG_STACK) {
                int64_t new_flag = DATARA_LIST_FLAG_HEAP;
                if (new_alloc_sz <= 1024) {
                    new_hdr = (DataraListHeader*)datara_rt_pool_alloc(new_alloc_sz);
                    new_flag = DATARA_LIST_FLAG_POOL;
                } else {
                    tls_heap_alloc_count++;
                    new_hdr = (DataraListHeader*)malloc(new_alloc_sz);
                }
                if (!new_hdr) return list;
                memcpy(new_hdr, hdr, sizeof(DataraListHeader) + (size_t)(count + 1) * sizeof(int64_t));
                new_hdr->magic = DATARA_LIST_MAGIC | new_flag;
                new_hdr->capacity = new_cap;
                hdr = new_hdr;
                list = (int64_t*)(hdr + 1);
            } else if (flag & DATARA_LIST_FLAG_POOL) {
                int64_t new_flag = DATARA_LIST_FLAG_HEAP;
                if (new_alloc_sz <= 1024) {
                    new_hdr = (DataraListHeader*)datara_rt_pool_alloc(new_alloc_sz);
                    new_flag = DATARA_LIST_FLAG_POOL;
                } else {
                    tls_heap_alloc_count++;
                    new_hdr = (DataraListHeader*)malloc(new_alloc_sz);
                }
                if (!new_hdr) return list;
                memcpy(new_hdr, hdr, sizeof(DataraListHeader) + (size_t)(count + 1) * sizeof(int64_t));
                size_t old_alloc_sz = sizeof(DataraListHeader) + (size_t)(hdr->capacity + 1) * sizeof(int64_t);
                datara_rt_pool_free(hdr, old_alloc_sz);
                new_hdr->magic = DATARA_LIST_MAGIC | new_flag;
                new_hdr->capacity = new_cap;
                hdr = new_hdr;
                list = (int64_t*)(hdr + 1);
            } else {
                new_hdr = (DataraListHeader*)realloc(hdr, new_alloc_sz);
                if (!new_hdr) return list;
                hdr = new_hdr;
                hdr->capacity = new_cap;
                list = (int64_t*)(hdr + 1);
            }
        }
        list[0] = count + 1;
        list[count + 1] = v;
        return list;
    }

    if (count > (int64_t)((SIZE_MAX - sizeof(DataraListHeader)) / (2 * sizeof(int64_t)) - 2)) return list;
    int64_t new_cap = (count + 1) * 2;
    if (new_cap < 8) new_cap = 8;
    DataraListHeader* new_hdr = (DataraListHeader*)malloc(sizeof(DataraListHeader) + (size_t)(new_cap + 1) * sizeof(int64_t));
    if (!new_hdr) return list;
    new_hdr->capacity = new_cap;
    new_hdr->magic = DATARA_LIST_MAGIC | DATARA_LIST_FLAG_HEAP;
    int64_t* new_list = (int64_t*)(new_hdr + 1);
    new_list[0] = count + 1;
    for (int64_t i = 1; i <= count; i++) {
        new_list[i] = list[i];
    }
    new_list[count + 1] = v;
    return new_list;
}

// v1.4.1: the checked pop lives in datara_rt_list_pop_outcome, which returns
// an Outcome<T> object instead of a bare element. The legacy bare-element
// pop is kept under a new name so stale callers fail at link time instead of
// silently reading an Outcome object as an element.
int64_t datara_rt_list_pop_legacy(int64_t* list) {
    if (!list || list[0] <= 0) return 0;
    int64_t count = list[0];
    int64_t val = list[count];
    list[0] = count - 1;
    return val;
}

int64_t datara_rt_list_pop(int64_t* list) {
    return datara_rt_list_pop_legacy(list);
}

int64_t* datara_rt_slice(int64_t* list, int64_t start, int64_t end) {
    if (!list) return NULL;
    int64_t len = list[0];
    if (len < 0) return NULL;
    if (start < 0) start = 0;
    if (end > len) end = len;
    if (start >= end) {
        return datara_rt_list_create_capacity(0);
    }
    int64_t count = end - start;
    if (count > (int64_t)((SIZE_MAX - sizeof(DataraListHeader)) / sizeof(int64_t) - 1)) return NULL;
    DataraListHeader* hdr = (DataraListHeader*)malloc(sizeof(DataraListHeader) + (size_t)(count + 1) * sizeof(int64_t));
    if (!hdr) return NULL;
    hdr->capacity = count;
    hdr->magic = DATARA_LIST_MAGIC;
    int64_t* res = (int64_t*)(hdr + 1);
    res[0] = count;
    for (int64_t i = 0; i < count; i++) {
        res[i + 1] = list[start + i + 1];
    }
    return res;
}

// ---------------------------------------------------------------------------
// v1.4.1: Full List<T> protocol runtime.
//
// Representation recap: element i lives at list[i + 1], the live count at
// list[0]. Float elements are stored as their IEEE-754 bit pattern in the
// i64 slot, string elements as char* pointers, class elements (List, Map,
// Outcome, ...) as object pointers with reference semantics.
//
// Never-free invariant (see datara_rt_str_free): runtime strings are never
// freed, so every operation below that copies or moves string elements
// copies the POINTER only -- no refcount traffic, no ownership transfer,
// nothing to release. Class elements are likewise copied as bare pointers.
// ---------------------------------------------------------------------------

// Stable in-place merge sort, O(n log n) time and O(n) scratch. mode 0 is
// the natural order. elem_kind selects the comparison: 0 = Int (direct i64
// compare), 1 = Float (i64 slots are IEEE bit patterns, compared as
// doubles), 2 = Str (lexicographic strcmp on the char* elements). Lists are
// statically homogeneous -- the type checker never admits a mixed List -- so
// elem_kind is validated to fail loudly rather than guess a layout.
static int datara_list_lt(int64_t a, int64_t b, int64_t elem_kind) {
    switch (elem_kind) {
        case 0:
            return a < b;
        case 1: {
            double fa, fb;
            memcpy(&fa, &a, sizeof(fa));
            memcpy(&fb, &b, sizeof(fb));
            return fa < fb;
        }
        case 2:
            return strcmp((const char*)(uintptr_t)a, (const char*)(uintptr_t)b) < 0;
        default:
            datara_rt_panic("list sort: unsupported element representation");
            return 0;
    }
}

static void datara_list_sort_range(int64_t* list, int64_t* tmp, int64_t lo, int64_t hi,
                                   int64_t elem_kind) {
    if (hi - lo <= 1) return;
    int64_t mid = lo + (hi - lo) / 2;
    datara_list_sort_range(list, tmp, lo, mid, elem_kind);
    datara_list_sort_range(list, tmp, mid, hi, elem_kind);
    // Merge with a strict `less`: equal elements take the left run first,
    // which is exactly the stability guarantee.
    int64_t i = lo, j = mid, k = lo;
    while (i < mid && j < hi) {
        if (datara_list_lt(list[j + 1], list[i + 1], elem_kind)) {
            tmp[k++] = list[++j];
        } else {
            tmp[k++] = list[++i];
        }
    }
    while (i < mid) tmp[k++] = list[++i];
    while (j < hi) tmp[k++] = list[++j];
    memcpy(&list[lo + 1], &tmp[lo], (size_t)(hi - lo) * sizeof(int64_t));
}

int64_t datara_rt_list_sort(int64_t* list, int64_t mode, int64_t elem_kind) {
    if (!list) return 0;
    if (mode != 0) {
        datara_rt_panic("list sort: unsupported sort mode");
        return 0;
    }
    if (elem_kind != 0 && elem_kind != 1 && elem_kind != 2) {
        datara_rt_panic("list sort: unsupported element representation");
        return 0;
    }
    int64_t count = list[0];
    if (count > 1) {
        int64_t* tmp = (int64_t*)malloc((size_t)count * sizeof(int64_t));
        if (!tmp) {
            datara_rt_panic("list sort: out of memory");
            return 0;
        }
        datara_list_sort_range(list, tmp, 0, count, elem_kind);
        free(tmp);
    }
    // In-place: the block never moves, so returning the same handle keeps
    // every caller alias valid (matches the set/push reassign contract).
    return (int64_t)(uintptr_t)list;
}

// Pointer-equality-aware membership helpers. elem_kind uses the same
// encoding as sort (0 = Int, 1 = Float, 2 = Str). Strings are compared by
// strcmp, never by pointer: distinct char* blocks can hold equal text.
static int datara_list_eq(int64_t a, int64_t b, int64_t elem_kind) {
    switch (elem_kind) {
        case 0:
        case 1:
            // Int slots and Float bit patterns compare directly; bits
            // equality is the correct Float equality in this representation.
            return a == b;
        case 2:
            return strcmp((const char*)(uintptr_t)a, (const char*)(uintptr_t)b) == 0;
        default:
            datara_rt_panic("list compare: unsupported element representation");
            return 0;
    }
}

int64_t datara_rt_list_index_of(int64_t* list, int64_t v, int64_t elem_kind) {
    if (!list) return -1;
    int64_t count = list[0];
    for (int64_t i = 0; i < count; i++) {
        if (datara_list_eq(list[i + 1], v, elem_kind)) return i;
    }
    return -1;
}

int64_t datara_rt_list_contains(int64_t* list, int64_t v, int64_t elem_kind) {
    return datara_rt_list_index_of(list, v, elem_kind) >= 0 ? 1 : 0;
}

// Removes the element at idx by moving the tail down. Returns 1 on success,
// 0 when idx is out of bounds -- an honest "nothing was removed", not a
// trap and not a silent success.
int64_t datara_rt_list_remove_at(int64_t* list, int64_t idx) {
    if (!list) return 0;
    int64_t count = list[0];
    if (idx < 0 || idx >= count) return 0;
    if (idx + 1 < count) {
        memmove(&list[idx + 1], &list[idx + 2], (size_t)(count - idx - 1) * sizeof(int64_t));
    }
    list[0] = count - 1;
    return 1;
}

// Removes the first occurrence of v (elem_kind-aware compare). Returns 1 if
// an element was removed, 0 when the value is absent.
int64_t datara_rt_list_remove_value(int64_t* list, int64_t v, int64_t elem_kind) {
    int64_t idx = datara_rt_list_index_of(list, v, elem_kind);
    if (idx < 0) return 0;
    return datara_rt_list_remove_at(list, idx);
}

// Inserts v at idx, shifting the tail up. idx == count appends. Returns the
// (possibly reallocated) list handle: the inserted case returns the new
// block, and an out-of-bounds idx returns the UNCHANGED receiver handle so
// the caller's reassign keeps a valid list (an honest no-op, never a
// poisoned NULL). Returning the handle is what makes growth safe: like
// datara_rt_list_set/append, callers reassign the receiver from the result
// instead of holding a stale block pointer.
int64_t datara_rt_list_insert_at(int64_t* list, int64_t idx, int64_t v) {
    if (!list) return 0;
    int64_t count = list[0];
    if (idx < 0 || idx > count) return (int64_t)(uintptr_t)list;
    DataraListHeader* hdr = ((DataraListHeader*)list) - 1;
    if (count < hdr->capacity) {
        if (idx < count) {
            memmove(&list[idx + 2], &list[idx + 1], (size_t)(count - idx) * sizeof(int64_t));
        }
        list[idx + 1] = v;
        list[0] = count + 1;
        return (int64_t)(uintptr_t)list;
    }
    // Full: migrate to a fresh block (append's growth contract). The old
    // block is abandoned for STACK sources (a caller frame owns it) and
    // returned to the pool for POOL sources.
    int64_t flag = hdr->magic & 0xFULL;
    int64_t new_cap = (count + 1) * 2;
    if (new_cap < 8) new_cap = 8;
    DataraListHeader* new_hdr =
        (DataraListHeader*)malloc(sizeof(DataraListHeader) + (size_t)(new_cap + 1) * sizeof(int64_t));
    if (!new_hdr) return 0;
    new_hdr->capacity = new_cap;
    new_hdr->magic = DATARA_LIST_MAGIC | DATARA_LIST_FLAG_HEAP;
    int64_t* dst = (int64_t*)(new_hdr + 1);
    dst[0] = count + 1;
    for (int64_t i = 0; i < idx; i++) dst[i + 1] = list[i + 1];
    dst[idx + 1] = v;
    for (int64_t i = idx; i < count; i++) dst[i + 2] = list[i + 1];
    if (flag & DATARA_LIST_FLAG_POOL) {
        datara_rt_pool_free(hdr, sizeof(DataraListHeader) + (size_t)(hdr->capacity + 1) * sizeof(int64_t));
    }
    // STACK/HEAP sources: STACK frames own their block, HEAP blocks follow
    // the documented leak-by-design policy for runtime lists in flight.
    return (int64_t)(uintptr_t)dst;
}

// Reverses in place. Float bit patterns and string pointers are moved
// bit-exactly (pointer copies only -- never-free invariant).
int64_t datara_rt_list_reverse(int64_t* list) {
    if (!list) return 0;
    int64_t count = list[0];
    for (int64_t i = 0, j = count - 1; i < j; i++, j--) {
        int64_t t = list[i + 1];
        list[i + 1] = list[j + 1];
        list[j + 1] = t;
    }
    return (int64_t)(uintptr_t)list;
}

// Drops every element. String elements are NOT freed -- the runtime never
// frees strings (never-free invariant), so clearing is just a count reset.
int64_t datara_rt_list_clear(int64_t* list) {
    if (!list) return 0;
    list[0] = 0;
    return (int64_t)(uintptr_t)list;
}

int64_t datara_rt_list_is_empty(int64_t* list) {
    return (!list || list[0] <= 0) ? 1 : 0;
}

// New slice sharing datara_rt_slice's clamping; string elements are copied
// as pointers (never-free invariant: the source strings outlive the copy).
int64_t* datara_rt_list_slice(int64_t* list, int64_t start, int64_t end) {
    return datara_rt_slice(list, start, end);
}

int64_t* datara_rt_list_create_repeat(int64_t elem, int64_t count) {
    if (count < 0) count = 0;
    int64_t* arr = datara_rt_list_create_capacity(count);
    if (!arr) return NULL;
    arr[0] = count;
    for (int64_t i = 0; i < count; i++) {
        arr[i + 1] = elem;
    }
    return arr;
}

static int64_t* datara_rt_list_create_from(const int64_t* vals, int64_t count) {
    int64_t cap = count < 8 ? 8 : count;
    int64_t* arr = datara_rt_list_create_capacity(cap);
    if (!arr) return NULL;
    arr[0] = count;
    for (int64_t i = 0; i < count; i++) {
        arr[i + 1] = vals[i];
    }
    return arr;
}

int64_t* datara_rt_list_create_1(int64_t a) {
    return datara_rt_list_create_from(&a, 1);
}

int64_t* datara_rt_list_create_2(int64_t a, int64_t b) {
    int64_t vals[2] = { a, b };
    return datara_rt_list_create_from(vals, 2);
}

int64_t* datara_rt_list_create_3(int64_t a, int64_t b, int64_t c) {
    int64_t vals[3] = { a, b, c };
    return datara_rt_list_create_from(vals, 3);
}

int64_t* datara_rt_list_create_4(int64_t a, int64_t b, int64_t c, int64_t d) {
    int64_t vals[4] = { a, b, c, d };
    return datara_rt_list_create_from(vals, 4);
}

int64_t* datara_rt_list_create_5(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e) {
    int64_t vals[5] = { a, b, c, d, e };
    return datara_rt_list_create_from(vals, 5);
}

