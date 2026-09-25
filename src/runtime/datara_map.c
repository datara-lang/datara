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

// M1 HashMap: the entry array stays insertion-ordered (an IndexMap
// contract), while get/contains go through a lazily built robin-hood
// open-addressing index for O(1) lookup. The index stores entry_index+1
// per slot (0 = empty), lives in its own malloc block referenced from the
// header, and is dropped/rebuilt whenever the entry array grows or the
// load factor exceeds 3/4 — below DATARA_MAP_INDEX_MIN entries a linear
// scan is faster than any hashing, so none is built.
typedef struct {
    int64_t capacity;
    int64_t magic;
    int64_t* index;
    int64_t index_slots;
} DataraMapHeader;

#define DATARA_MAP_INDEX_MIN 16

// M1 growth-soundness: map keys are OWNED COPIES. Callers routinely pass
// strings living in the recycled scratch ring (e.g. fmt"key_{i}" in a
// loop); storing those pointers verbatim dangles every key once the ring
// wraps (~1MB of allocations), which silently froze map growth.
static const char* rt_map_own_key(const char* key) {
    if (!key) return NULL;
    size_t n = strlen(key) + 1;
    char* copy = (char*)malloc(n);
    if (!copy) return key; // OOM fallback keeps the old (unsafe) behavior
    memcpy(copy, key, n);
    return copy;
}

static uint64_t rt_map_hash(const char* key) {
    uint64_t h = 1469598103934665603ULL; // FNV-1a 64
    for (const unsigned char* p = (const unsigned char*)key; *p; p++) {
        h ^= (uint64_t)*p;
        h *= 1099511628211ULL;
    }
    return h;
}

static void rt_map_index_free(DataraMapHeader* hdr) {
    if (hdr->index) {
        free(hdr->index);
        hdr->index = NULL;
    }
    hdr->index_slots = 0;
}

static void rt_map_index_put(int64_t* map, DataraMapHeader* hdr, int64_t entry_idx) {
    int64_t mask = hdr->index_slots - 1;
    int64_t* idx = hdr->index;
    const char* k = (const char*)map[1 + entry_idx * 2];
    int64_t home = (int64_t)(rt_map_hash(k) & (uint64_t)mask);
    int64_t slot = home;
    int64_t pend_e = entry_idx;
    int64_t pend_dist = 0;
    for (;;) {
        int64_t cur = idx[slot];
        if (cur == 0) {
            idx[slot] = pend_e + 1;
            return;
        }
        int64_t cur_e = cur - 1;
        const char* ck = (const char*)map[1 + cur_e * 2];
        int64_t cur_home = (int64_t)(rt_map_hash(ck) & (uint64_t)mask);
        int64_t cur_dist = slot - cur_home;
        if (cur_dist < 0) cur_dist += hdr->index_slots;
        // Robin hood: a richer element (larger probe distance) takes the
        // slot; the poorer one keeps probing from here.
        if (pend_dist > cur_dist) {
            idx[slot] = pend_e + 1;
            pend_e = cur_e;
            pend_dist = cur_dist;
        }
        slot = (slot + 1) & mask;
        pend_dist++;
    }
}

static void rt_map_index_build(int64_t* map, DataraMapHeader* hdr) {
    int64_t count = map[0];
    int64_t slots = DATARA_MAP_INDEX_MIN;
    while (slots < count * 2) slots <<= 1;
    rt_map_index_free(hdr);
    int64_t* idx = (int64_t*)calloc((size_t)slots, sizeof(int64_t));
    if (!idx) return; // stay on linear scan when out of memory
    hdr->index = idx;
    hdr->index_slots = slots;
    for (int64_t e = 0; e < count; e++) {
        rt_map_index_put(map, hdr, e);
    }
}

static int64_t rt_map_index_find(int64_t* map, DataraMapHeader* hdr, const char* key) {
    int64_t mask = hdr->index_slots - 1;
    int64_t slot = (int64_t)(rt_map_hash(key) & (uint64_t)mask);
    for (;;) {
        int64_t cur = hdr->index[slot];
        if (cur == 0) return -1;
        int64_t e = cur - 1;
        const char* ck = (const char*)map[1 + e * 2];
        if (ck && strcmp(ck, key) == 0) return e;
        slot = (slot + 1) & mask;
    }
}

#define DATARA_MAP_MAGIC      0x4441544D41503130ULL
#define DATARA_MAP_MAGIC_MASK 0xFFFFFFFFFFFFFFF0ULL
#define DATARA_MAP_FLAG_HEAP  0x0ULL
#define DATARA_MAP_FLAG_POOL  0x1ULL

void* datara_rt_map_create(void) {
    size_t init_cap = 8;
    size_t total_bytes = sizeof(DataraMapHeader) + (1 + init_cap * 2) * sizeof(int64_t);
    DataraMapHeader* hdr = NULL;
    int64_t flag = DATARA_MAP_FLAG_HEAP;
    if (total_bytes <= 1024) {
        hdr = (DataraMapHeader*)datara_rt_pool_alloc(total_bytes);
        flag = DATARA_MAP_FLAG_POOL;
    } else {
        tls_heap_alloc_count++;
        hdr = (DataraMapHeader*)malloc(total_bytes);
    }
    if (!hdr) return NULL;
    hdr->capacity = (int64_t)init_cap;
    hdr->magic = DATARA_MAP_MAGIC | flag;
    hdr->index = NULL;
    hdr->index_slots = 0;
    int64_t* map = (int64_t*)(hdr + 1);
    map[0] = 0;
    return map;
}

int64_t* datara_rt_map_create_2(const char* k0, int64_t v0, const char* k1, int64_t v1) {
    size_t init_cap = 8;
    size_t total_bytes = sizeof(DataraMapHeader) + (1 + init_cap * 2) * sizeof(int64_t);
    DataraMapHeader* hdr = (DataraMapHeader*)malloc(total_bytes);
    if (!hdr) return NULL;
    hdr->capacity = init_cap;
    hdr->magic = DATARA_MAP_MAGIC;
    hdr->index = NULL;
    hdr->index_slots = 0;
    int64_t* map = (int64_t*)(hdr + 1);
    map[0] = 2;
    map[1] = (int64_t)k0;
    map[2] = v0;
    map[3] = (int64_t)k1;
    map[4] = v1;
    return map;
}

static int64_t* datara_rt_map_create_from(const char** keys, const int64_t* vals, int64_t n) {
    size_t init_cap = 8;
    if ((size_t)n > init_cap) init_cap = (size_t)n;
    size_t total_bytes = sizeof(DataraMapHeader) + (1 + init_cap * 2) * sizeof(int64_t);
    DataraMapHeader* hdr = (DataraMapHeader*)malloc(total_bytes);
    if (!hdr) return NULL;
    hdr->capacity = init_cap;
    hdr->magic = DATARA_MAP_MAGIC;
    hdr->index = NULL;
    hdr->index_slots = 0;
    int64_t* map = (int64_t*)(hdr + 1);
    map[0] = n;
    for (int64_t i = 0; i < n; i++) {
        map[1 + i * 2] = (int64_t)keys[i];
        map[2 + i * 2] = vals[i];
    }
    return map;
}

void* datara_rt_map_create_1(const char* k0, int64_t v0) {
    const char* keys[1] = { k0 };
    const int64_t vals[1] = { v0 };
    return datara_rt_map_create_from(keys, vals, 1);
}

int64_t* datara_rt_map_create_3(const char* k0, int64_t v0, const char* k1, int64_t v1,
                                const char* k2, int64_t v2) {
    const char* keys[3] = { k0, k1, k2 };
    const int64_t vals[3] = { v0, v1, v2 };
    return datara_rt_map_create_from(keys, vals, 3);
}

int64_t* datara_rt_map_create_4(const char* k0, int64_t v0, const char* k1, int64_t v1,
                                const char* k2, int64_t v2, const char* k3, int64_t v3) {
    const char* keys[4] = { k0, k1, k2, k3 };
    const int64_t vals[4] = { v0, v1, v2, v3 };
    return datara_rt_map_create_from(keys, vals, 4);
}

int64_t* datara_rt_map_create_5(const char* k0, int64_t v0, const char* k1, int64_t v1,
                                const char* k2, int64_t v2, const char* k3, int64_t v3,
                                const char* k4, int64_t v4) {
    const char* keys[5] = { k0, k1, k2, k3, k4 };
    const int64_t vals[5] = { v0, v1, v2, v3, v4 };
    return datara_rt_map_create_from(keys, vals, 5);
}

static DataraMapHeader* datara_rt_map_get_header_fwd(int64_t* map);

int64_t datara_rt_map_get(int64_t* map, const char* key) {
    if (!map || !key) return 0;
    int64_t count = map[0];
    DataraMapHeader* hdr = datara_rt_map_get_header_fwd(map);
    if (hdr && hdr->index && hdr->index_slots && count >= DATARA_MAP_INDEX_MIN) {
        int64_t e = rt_map_index_find(map, hdr, key);
        return e >= 0 ? map[2 + e * 2] : 0;
    }
    for (int64_t i = 0; i < count; i++) {
        const char* k = (const char*)map[1 + i * 2];
        if (k && strcmp(k, key) == 0) {
            return map[2 + i * 2];
        }
    }
    return 0;
}

static inline DataraMapHeader* datara_rt_map_get_header(int64_t* map) {
    if (!map) return NULL;
    DataraMapHeader* hdr = ((DataraMapHeader*)map) - 1;
    if ((hdr->magic & DATARA_MAP_MAGIC_MASK) == DATARA_MAP_MAGIC) return hdr;
    return NULL;
}

static DataraMapHeader* datara_rt_map_get_header_fwd(int64_t* map) {
    return datara_rt_map_get_header(map);
}

int64_t* datara_rt_map_insert(int64_t* map, const char* key, int64_t val) {
    if (!map) {
        size_t init_cap = 8;
        size_t total_bytes = sizeof(DataraMapHeader) + (1 + init_cap * 2) * sizeof(int64_t);
        DataraMapHeader* hdr = (DataraMapHeader*)malloc(total_bytes);
        if (!hdr) return NULL;
        hdr->capacity = init_cap;
        hdr->magic = DATARA_MAP_MAGIC;
        hdr->index = NULL;
        hdr->index_slots = 0;
        int64_t* m = (int64_t*)(hdr + 1);
        m[0] = 1;
        m[1] = (int64_t)rt_map_own_key(key);
        m[2] = val;
        return m;
    }
    int64_t count = map[0];
    for (int64_t i = 0; i < count; i++) {
        const char* k = (const char*)map[1 + i * 2];
        if (k && key && strcmp(k, key) == 0) {
            map[2 + i * 2] = val;
            return map;
        }
    }
    DataraMapHeader* hdr = datara_rt_map_get_header(map);
    if (hdr) {
        if (count < hdr->capacity) {
            map[1 + count * 2] = (int64_t)rt_map_own_key(key);
            map[2 + count * 2] = val;
            map[0] = count + 1;
            // Keep the index in sync; drop it when the load factor would
            // exceed 3/4 — the next lookup rebuilds it at the new size.
            if (hdr->index && hdr->index_slots) {
                if ((count + 1) * 4 < hdr->index_slots * 3) {
                    rt_map_index_put(map, hdr, count);
                } else {
                    rt_map_index_free(hdr);
                }
            } else if (count + 1 >= DATARA_MAP_INDEX_MIN) {
                rt_map_index_build(map, hdr);
            }
            return map;
        }
        // Growth invalidates nothing structurally (slot values are entry
        // indices), but the index block is released so the next lookup
        // rebuilds against the new capacity.
        rt_map_index_free(hdr);
        int64_t new_cap = hdr->capacity * 2;
        size_t total_bytes = sizeof(DataraMapHeader) + (1 + new_cap * 2) * sizeof(int64_t);
        DataraMapHeader* new_hdr = NULL;
        if ((hdr->magic & 0xFULL) == DATARA_MAP_FLAG_POOL) {
            int64_t flag = DATARA_MAP_FLAG_HEAP;
            if (total_bytes <= 1024) {
                new_hdr = (DataraMapHeader*)datara_rt_pool_alloc(total_bytes);
                flag = DATARA_MAP_FLAG_POOL;
            } else {
                tls_heap_alloc_count++;
                new_hdr = (DataraMapHeader*)malloc(total_bytes);
            }
            if (!new_hdr) return map;
            memcpy(new_hdr, hdr, sizeof(DataraMapHeader) + (1 + (size_t)count * 2) * sizeof(int64_t));
            size_t old_total_bytes = sizeof(DataraMapHeader) + (1 + (size_t)hdr->capacity * 2) * sizeof(int64_t);
            datara_rt_pool_free(hdr, old_total_bytes);
            new_hdr->magic = DATARA_MAP_MAGIC | flag;
            new_hdr->capacity = new_cap;
        } else {
            // M4 growth-soundness: realloc may extend IN PLACE and hand
            // back the SAME pointer. The compiler's rebinding of the map
            // variable is then a no-op store, and the observed freeze was
            // 'len' stuck at the old count. Always move to a fresh block.
            new_hdr = (DataraMapHeader*)malloc(total_bytes);
            if (!new_hdr) return map;
            memcpy(new_hdr, hdr, sizeof(DataraMapHeader) + (1 + (size_t)count * 2) * sizeof(int64_t));
            free(hdr);
            new_hdr->capacity = new_cap;
        }
        int64_t* new_map = (int64_t*)(new_hdr + 1);
        new_map[1 + count * 2] = (int64_t)rt_map_own_key(key);
        new_map[2 + count * 2] = val;
        new_map[0] = count + 1;
        return new_map;
    } else {
        size_t init_cap = (count + 1) < 8 ? 8 : (count + 1) * 2;
        size_t total_bytes = sizeof(DataraMapHeader) + (1 + init_cap * 2) * sizeof(int64_t);
        DataraMapHeader* new_hdr = (DataraMapHeader*)malloc(total_bytes);
        if (!new_hdr) return map;
        new_hdr->capacity = init_cap;
        new_hdr->magic = DATARA_MAP_MAGIC;
        new_hdr->index = NULL;
        new_hdr->index_slots = 0;
        int64_t* new_map = (int64_t*)(new_hdr + 1);
        new_map[0] = count + 1;
        for (int64_t i = 0; i < count; i++) {
            new_map[1 + i * 2] = map[1 + i * 2];
            new_map[2 + i * 2] = map[2 + i * 2];
        }
        new_map[1 + count * 2] = (int64_t)rt_map_own_key(key);
        new_map[2 + count * 2] = val;
        free(map);
        return new_map;
    }
}

void datara_rt_map_free(void* map) {
    if (!map) return;
    DataraMapHeader* hdr = datara_rt_map_get_header((int64_t*)map);
    if (hdr) {
        rt_map_index_free(hdr);
        if ((hdr->magic & 0xFULL) == DATARA_MAP_FLAG_POOL) {
            size_t total_bytes = sizeof(DataraMapHeader) + (1 + (size_t)hdr->capacity * 2) * sizeof(int64_t);
            datara_rt_pool_free(hdr, total_bytes);
        } else {
            free(hdr);
        }
    } else {
        free(map);
    }
}

int64_t datara_rt_map_contains(int64_t* map, const char* key) {
    if (!map || !key) return 0;
    int64_t count = map[0];
    DataraMapHeader* hdr = datara_rt_map_get_header_fwd(map);
    if (hdr && hdr->index && hdr->index_slots && count >= DATARA_MAP_INDEX_MIN) {
        return rt_map_index_find(map, hdr, key) >= 0 ? 1 : 0;
    }
    for (int64_t i = 0; i < count; i++) {
        const char* k = (const char*)map[1 + i * 2];
        if (k && strcmp(k, key) == 0) {
            return 1;
        }
    }
    return 0;
}

int64_t datara_rt_map_len(int64_t* map) {
    if (!map) return 0;
    return map[0];
}

const char* datara_rt_range_str(int64_t start, int64_t end) {
    char* buf = (char*)malloc(48);
    if (!buf) return "";
    size_t l1 = fast_i64toa(start, buf);
    buf[l1] = '.';
    buf[l1 + 1] = '.';
    fast_i64toa(end, buf + l1 + 2);
    return buf;
}
