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

