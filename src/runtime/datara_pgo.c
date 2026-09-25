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
// Phase 16: Profile-Guided Optimization (PGO) Runtime Instrumentation
// ============================================================================
#define DATARA_PGO_MAX_FUNCS 4096
#define DATARA_PGO_MAX_BRANCHES 8192
#define DATARA_PGO_MAX_LOOPS 4096

typedef struct {
    char name[128];
    uint64_t count;
} DataraPgoFunc;

typedef struct {
    char id[128];
    uint64_t taken;
    uint64_t total;
} DataraPgoBranch;

typedef struct {
    char id[128];
    uint64_t trip_count;
} DataraPgoLoop;

static DataraPgoFunc g_pgo_funcs[DATARA_PGO_MAX_FUNCS];
static size_t g_pgo_func_count = 0;

static DataraPgoBranch g_pgo_branches[DATARA_PGO_MAX_BRANCHES];
static size_t g_pgo_branch_count = 0;

static DataraPgoLoop g_pgo_loops[DATARA_PGO_MAX_LOOPS];
static size_t g_pgo_loop_count = 0;

static char g_pgo_output_path[512] = {0};
static int g_pgo_atexit_registered = 0;

#if defined(_WIN32)
static SRWLOCK g_pgo_lock = SRWLOCK_INIT;
#define PGO_LOCK() AcquireSRWLockExclusive(&g_pgo_lock)
#define PGO_UNLOCK() ReleaseSRWLockExclusive(&g_pgo_lock)
#else
static pthread_mutex_t g_pgo_lock = PTHREAD_MUTEX_INITIALIZER;
#define PGO_LOCK() pthread_mutex_lock(&g_pgo_lock)
#define PGO_UNLOCK() pthread_mutex_unlock(&g_pgo_lock)
#endif

static void datara_rt_pgo_ensure_parent_dirs(const char* path) {
    if (!path) return;
    char tmp[512];
    strncpy(tmp, path, sizeof(tmp) - 1);
    tmp[sizeof(tmp) - 1] = '\0';
    for (char* p = tmp + 1; *p; p++) {
        if (*p == '/' || *p == '\\') {
            char ch = *p;
            *p = '\0';
#ifdef _WIN32
            CreateDirectoryA(tmp, NULL);
#else
            mkdir(tmp, 0755);
#endif
            *p = ch;
        }
    }
}

void datara_rt_pgo_auto_flush(void) {
    if (g_pgo_func_count == 0 && g_pgo_branch_count == 0 && g_pgo_loop_count == 0) {
        return;
    }
    const char* path = g_pgo_output_path;
    if (!path || path[0] == '\0') {
        path = getenv("DATARA_PGO_FILE");
    }
    if (!path || path[0] == '\0') {
        path = getenv("FORGEN_PGO_FILE");
    }
    if (!path || path[0] == '\0') {
        path = "app.profdata";
    }
    datara_rt_pgo_flush(path);
}

void datara_rt_pgo_set_output_file(const char* path) {
    if (!path) return;
    PGO_LOCK();
    strncpy(g_pgo_output_path, path, sizeof(g_pgo_output_path) - 1);
    g_pgo_output_path[sizeof(g_pgo_output_path) - 1] = '\0';
    if (!g_pgo_atexit_registered) {
        atexit(datara_rt_pgo_auto_flush);
        g_pgo_atexit_registered = 1;
    }
    PGO_UNLOCK();
}

void datara_rt_pgo_hit_func(const char* name) {
    if (!name || name[0] == '\0') return;
    PGO_LOCK();
    if (!g_pgo_atexit_registered) {
        atexit(datara_rt_pgo_auto_flush);
        g_pgo_atexit_registered = 1;
    }
    for (size_t i = 0; i < g_pgo_func_count; i++) {
        if (strcmp(g_pgo_funcs[i].name, name) == 0) {
            g_pgo_funcs[i].count++;
            PGO_UNLOCK();
            return;
        }
    }
    if (g_pgo_func_count < DATARA_PGO_MAX_FUNCS) {
        strncpy(g_pgo_funcs[g_pgo_func_count].name, name, sizeof(g_pgo_funcs[0].name) - 1);
        g_pgo_funcs[g_pgo_func_count].name[sizeof(g_pgo_funcs[0].name) - 1] = '\0';
        g_pgo_funcs[g_pgo_func_count].count = 1;
        g_pgo_func_count++;
    }
    PGO_UNLOCK();
}

void datara_rt_pgo_hit_branch(const char* branch_id, int64_t taken) {
    if (!branch_id || branch_id[0] == '\0') return;
    PGO_LOCK();
    if (!g_pgo_atexit_registered) {
        atexit(datara_rt_pgo_auto_flush);
        g_pgo_atexit_registered = 1;
    }
    for (size_t i = 0; i < g_pgo_branch_count; i++) {
        if (strcmp(g_pgo_branches[i].id, branch_id) == 0) {
            g_pgo_branches[i].total++;
            if (taken != 0) {
                g_pgo_branches[i].taken++;
            }
            PGO_UNLOCK();
            return;
        }
    }
    if (g_pgo_branch_count < DATARA_PGO_MAX_BRANCHES) {
        strncpy(g_pgo_branches[g_pgo_branch_count].id, branch_id, sizeof(g_pgo_branches[0].id) - 1);
        g_pgo_branches[g_pgo_branch_count].id[sizeof(g_pgo_branches[0].id) - 1] = '\0';
        g_pgo_branches[g_pgo_branch_count].total = 1;
        g_pgo_branches[g_pgo_branch_count].taken = (taken != 0) ? 1 : 0;
        g_pgo_branch_count++;
    }
    PGO_UNLOCK();
}

void datara_rt_pgo_hit_loop(const char* loop_id, int64_t trip_count) {
    if (!loop_id || loop_id[0] == '\0') return;
    PGO_LOCK();
    if (!g_pgo_atexit_registered) {
        atexit(datara_rt_pgo_auto_flush);
        g_pgo_atexit_registered = 1;
    }
    for (size_t i = 0; i < g_pgo_loop_count; i++) {
        if (strcmp(g_pgo_loops[i].id, loop_id) == 0) {
            g_pgo_loops[i].trip_count += (uint64_t)trip_count;
            PGO_UNLOCK();
            return;
        }
    }
    if (g_pgo_loop_count < DATARA_PGO_MAX_LOOPS) {
        strncpy(g_pgo_loops[g_pgo_loop_count].id, loop_id, sizeof(g_pgo_loops[0].id) - 1);
        g_pgo_loops[g_pgo_loop_count].id[sizeof(g_pgo_loops[0].id) - 1] = '\0';
        g_pgo_loops[g_pgo_loop_count].trip_count = (uint64_t)trip_count;
        g_pgo_loop_count++;
    }
    PGO_UNLOCK();
}

void datara_rt_pgo_reset(void) {
    PGO_LOCK();
    g_pgo_func_count = 0;
    g_pgo_branch_count = 0;
    g_pgo_loop_count = 0;
    g_pgo_output_path[0] = '\0';
    PGO_UNLOCK();
}

void datara_rt_pgo_flush(const char* path) {
    if (!path || path[0] == '\0') {
        path = g_pgo_output_path;
    }
    if (!path || path[0] == '\0') {
        path = getenv("DATARA_PGO_FILE");
    }
    if (!path || path[0] == '\0') {
        path = getenv("FORGEN_PGO_FILE");
    }
    if (!path || path[0] == '\0') {
        path = "app.profdata";
    }

    PGO_LOCK();
    datara_rt_pgo_ensure_parent_dirs(path);
    FILE* f = fopen(path, "w");
    if (!f) {
        PGO_UNLOCK();
        return;
    }

    fprintf(f, "{\n");
    fprintf(f, "  \"project_name\": \"datara_pgo\",\n");
    fprintf(f, "  \"source\": \"runtime\",\n");
    fprintf(f, "  \"hot_functions\": {\n");
    for (size_t i = 0; i < g_pgo_func_count; i++) {
        fprintf(f, "    \"%s\": %llu%s\n",
            g_pgo_funcs[i].name,
            (unsigned long long)g_pgo_funcs[i].count,
            (i + 1 < g_pgo_func_count) ? "," : "");
    }
    fprintf(f, "  },\n");
    fprintf(f, "  \"branch_frequencies\": {\n");
    for (size_t i = 0; i < g_pgo_branch_count; i++) {
        fprintf(f, "    \"%s\": [%llu, %llu]%s\n",
            g_pgo_branches[i].id,
            (unsigned long long)g_pgo_branches[i].taken,
            (unsigned long long)g_pgo_branches[i].total,
            (i + 1 < g_pgo_branch_count) ? "," : "");
    }
    fprintf(f, "  },\n");
    fprintf(f, "  \"loop_trip_counts\": {\n");
    for (size_t i = 0; i < g_pgo_loop_count; i++) {
        fprintf(f, "    \"%s\": %llu%s\n",
            g_pgo_loops[i].id,
            (unsigned long long)g_pgo_loops[i].trip_count,
            (i + 1 < g_pgo_loop_count) ? "," : "");
    }
    fprintf(f, "  },\n");
    fprintf(f, "  \"allocation_hotspots\": {},\n");
    fprintf(f, "  \"type_feedback\": {}\n");
    fprintf(f, "}\n");
    fclose(f);
    PGO_UNLOCK();
}

