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

typedef struct {
    int64_t capacity;
    int64_t magic;
} DataraMapHeader;

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

int64_t datara_rt_map_get(int64_t* map, const char* key) {
    if (!map || !key) return 0;
    int64_t count = map[0];
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

int64_t* datara_rt_map_insert(int64_t* map, const char* key, int64_t val) {
    if (!map) {
        size_t init_cap = 8;
        size_t total_bytes = sizeof(DataraMapHeader) + (1 + init_cap * 2) * sizeof(int64_t);
        DataraMapHeader* hdr = (DataraMapHeader*)malloc(total_bytes);
        if (!hdr) return NULL;
        hdr->capacity = init_cap;
        hdr->magic = DATARA_MAP_MAGIC;
        int64_t* m = (int64_t*)(hdr + 1);
        m[0] = 1;
        m[1] = (int64_t)key;
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
            map[1 + count * 2] = (int64_t)key;
            map[2 + count * 2] = val;
            map[0] = count + 1;
            return map;
        }
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
            new_hdr = (DataraMapHeader*)realloc(hdr, total_bytes);
            if (!new_hdr) return map;
            new_hdr->capacity = new_cap;
        }
        int64_t* new_map = (int64_t*)(new_hdr + 1);
        new_map[1 + count * 2] = (int64_t)key;
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
        int64_t* new_map = (int64_t*)(new_hdr + 1);
        new_map[0] = count + 1;
        for (int64_t i = 0; i < count; i++) {
            new_map[1 + i * 2] = map[1 + i * 2];
            new_map[2 + i * 2] = map[2 + i * 2];
        }
        new_map[1 + count * 2] = (int64_t)key;
        new_map[2 + count * 2] = val;
        free(map);
        return new_map;
    }
}

void datara_rt_map_free(void* map) {
    if (!map) return;
    DataraMapHeader* hdr = datara_rt_map_get_header((int64_t*)map);
    if (hdr) {
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
