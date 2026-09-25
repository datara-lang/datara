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
// ---------------------------------------------------------------------------
// Memory Management & RAII
// ---------------------------------------------------------------------------

void datara_rt_free(void* ptr) {
    if (!ptr) {
        return;
    }
    // P1: the string scratch ring now serves every runtime string producer
    // (float_to_str, sanitize_utf8, concat, ...). Freeing a pointer into the
    // single TLS ring allocation would corrupt the heap for the whole thread,
    // so pointers inside the ring are owned by the ring and ignored here —
    // same contract as datara_rt_str_free. Genuine heap blocks are freed.
    if (datara_str_in_scratch_ring(ptr)) {
        return;
    }
    free(ptr);
}

// Strings produced by the runtime live in TLS static buffers (tls_int_bufs),
// inside the shared tls_scratch_ring block, or are heap blocks that the
// runtime intentionally never frees (see the note near datara_rt_bool_to_str).
// Generated code calls str_free speculatively, so there is no ownership
// tracking that would let us prove a pointer is a plain heap allocation.
// Freeing any of the above would corrupt the heap, so this is a documented
// no-op: runtime strings leak (by design), but nothing is ever corrupted.
// Lists and maps carry tracked headers and are freed by their own *_free.
void datara_rt_str_free(const char* s) {
    (void)s;
}

void datara_rt_list_free(void* list) {
    if (!list) return;
    DataraListHeader* hdr = ((DataraListHeader*)list) - 1;
    if ((hdr->magic & DATARA_LIST_MAGIC_MASK) == DATARA_LIST_MAGIC) {
        int64_t flag = hdr->magic & 0xFULL;
        if (flag & DATARA_LIST_FLAG_STACK) {
            return;
        }
        if (flag & DATARA_LIST_FLAG_POOL) {
            size_t alloc_sz = sizeof(DataraListHeader) + (size_t)(hdr->capacity + 1) * sizeof(int64_t);
            datara_rt_pool_free(hdr, alloc_sz);
            return;
        }
        free(hdr);
    } else {
        free(list);
    }
}

