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
// High-Performance Multithreading Engine: Hardware-scaling Thread Pool
// ---------------------------------------------------------------------------

#define DATARA_MAX_WORKERS 64

typedef struct {
    void (*fn)(int64_t, void*);
    void* ctx;
    int64_t start;
    int64_t end;
} DataraParallelChunk;

typedef struct {
    void (*task_fn)(void*);
    void* task_ctx;
    volatile long is_done;
} DataraTask;

static int g_workers_count = 0;
static int g_workers_initialized = 0;

static volatile int64_t g_par_cur = 0;
static int64_t g_par_end = 0;
static int64_t g_par_chunk = 1;
static void (*g_par_fn)(int64_t, void*) = NULL;
static void* g_par_ctx = NULL;

static void datara_rt_run_dynamic_loop(void) {
    int64_t chunk = g_par_chunk;
    int64_t end = g_par_end;
    void (*fn)(int64_t, void*) = g_par_fn;
    void* ctx = g_par_ctx;
    if (!fn) return;

    while (1) {
#ifdef _WIN32
        int64_t my_start = InterlockedAdd64(&g_par_cur, chunk) - chunk;
#else
        int64_t my_start = __sync_fetch_and_add(&g_par_cur, chunk);
#endif
        if (my_start >= end) break;
        int64_t my_end = my_start + chunk;
        if (my_end > end) my_end = end;
        for (int64_t i = my_start; i < my_end; i++) {
            fn(i, ctx);
        }
    }
}

#ifdef _WIN32
static HANDLE g_worker_threads[DATARA_MAX_WORKERS];
static HANDLE g_start_events[DATARA_MAX_WORKERS];
static HANDLE g_done_events[DATARA_MAX_WORKERS];
static volatile int g_shutdown = 0;
static DataraParallelChunk g_worker_chunks[DATARA_MAX_WORKERS];
static DataraTask g_worker_tasks[DATARA_MAX_WORKERS];
static volatile int g_worker_mode[DATARA_MAX_WORKERS]; // 0 = none, 1 = parallel_for, 2 = invoke

static DWORD WINAPI datara_worker_proc(LPVOID arg) {
    int worker_idx = (int)(intptr_t)arg;
    while (1) {
        WaitForSingleObject(g_start_events[worker_idx], INFINITE);
        if (g_shutdown) break;

        if (g_worker_mode[worker_idx] == 1) {
            datara_rt_run_dynamic_loop();
        } else if (g_worker_mode[worker_idx] == 2) {
            DataraTask* t = &g_worker_tasks[worker_idx];
            if (t->task_fn) {
                t->task_fn(t->task_ctx);
            }
            t->is_done = 1;
        }

        datara_rt_flush();
        SetEvent(g_done_events[worker_idx]);
    }
    return 0;
}
#else
#include <pthread.h>
#include <unistd.h>
static pthread_t g_worker_threads[DATARA_MAX_WORKERS];
static pthread_mutex_t g_worker_mutexes[DATARA_MAX_WORKERS];
static pthread_cond_t g_worker_conds[DATARA_MAX_WORKERS];
static volatile int g_worker_ready[DATARA_MAX_WORKERS];
static volatile int g_worker_done[DATARA_MAX_WORKERS];
static volatile int g_shutdown = 0;
static DataraParallelChunk g_worker_chunks[DATARA_MAX_WORKERS];
static DataraTask g_worker_tasks[DATARA_MAX_WORKERS];
static volatile int g_worker_mode[DATARA_MAX_WORKERS];

static void* datara_worker_proc(void* arg) {
    int worker_idx = (int)(intptr_t)arg;
    while (1) {
        pthread_mutex_lock(&g_worker_mutexes[worker_idx]);
        while (!g_worker_ready[worker_idx] && !g_shutdown) {
            pthread_cond_wait(&g_worker_conds[worker_idx], &g_worker_mutexes[worker_idx]);
        }
        if (g_shutdown) {
            pthread_mutex_unlock(&g_worker_mutexes[worker_idx]);
            break;
        }
        g_worker_ready[worker_idx] = 0;
        pthread_mutex_unlock(&g_worker_mutexes[worker_idx]);

        if (g_worker_mode[worker_idx] == 1) {
            datara_rt_run_dynamic_loop();
        } else if (g_worker_mode[worker_idx] == 2) {
            DataraTask* t = &g_worker_tasks[worker_idx];
            if (t->task_fn) {
                t->task_fn(t->task_ctx);
            }
            t->is_done = 1;
        }

        datara_rt_flush();
        pthread_mutex_lock(&g_worker_mutexes[worker_idx]);
        g_worker_done[worker_idx] = 1;
        pthread_cond_signal(&g_worker_conds[worker_idx]);
        pthread_mutex_unlock(&g_worker_mutexes[worker_idx]);
    }
    return NULL;
}
#endif

void datara_rt_thread_pool_init(int64_t workers) {
    if (g_workers_initialized) return;
    if (workers <= 0) {
#ifdef _WIN32
        SYSTEM_INFO sys;
        GetSystemInfo(&sys);
        workers = (int64_t)sys.dwNumberOfProcessors;
#else
        workers = (int64_t)sysconf(_SC_NPROCESSORS_ONLN);
#endif
    }
    if (workers > DATARA_MAX_WORKERS) workers = DATARA_MAX_WORKERS;
    if (workers < 1) workers = 1;
    g_workers_count = (int)workers;

#ifdef _WIN32
    for (int i = 1; i < g_workers_count; i++) {
        g_start_events[i] = CreateEvent(NULL, FALSE, FALSE, NULL);
        g_done_events[i] = CreateEvent(NULL, FALSE, FALSE, NULL);
        g_worker_threads[i] = CreateThread(NULL, 0, datara_worker_proc, (LPVOID)(intptr_t)i, 0, NULL);
    }
#else
    for (int i = 1; i < g_workers_count; i++) {
        pthread_mutex_init(&g_worker_mutexes[i], NULL);
        pthread_cond_init(&g_worker_conds[i], NULL);
        g_worker_ready[i] = 0;
        g_worker_done[i] = 0;
        pthread_create(&g_worker_threads[i], NULL, datara_worker_proc, (void*)(intptr_t)i);
    }
#endif
    g_workers_initialized = 1;
}

static inline void datara_rt_ensure_threads(void) {
    if (!g_workers_initialized) {
        datara_rt_thread_pool_init(0);
    }
}

int64_t datara_rt_num_workers(void) {
    datara_rt_ensure_threads();
    return (int64_t)g_workers_count;
}

void datara_rt_pin_worker_threads(void) {
    datara_rt_ensure_threads();
#ifdef _WIN32
    for (int i = 1; i < g_workers_count; i++) {
        if (g_worker_threads[i]) {
            DWORD_PTR mask = (DWORD_PTR)1 << (i % 64);
            SetThreadAffinityMask(g_worker_threads[i], mask);
        }
    }
#elif !defined(__APPLE__)
    for (int i = 1; i < g_workers_count; i++) {
        cpu_set_t cpuset;
        CPU_ZERO(&cpuset);
        CPU_SET(i, &cpuset);
        pthread_setaffinity_np(g_worker_threads[i], sizeof(cpu_set_t), &cpuset);
    }
#endif
}

static DATARA_TLS int tls_in_parallel_region = 0;

void datara_rt_parallel_for(int64_t start, int64_t end, void (*fn)(int64_t idx, void* ctx), void* ctx) {
    if (start >= end || !fn) return;
    datara_rt_ensure_threads();

    int64_t total = end - start;
    int num_w = g_workers_count;
    if (tls_in_parallel_region != 0 || num_w <= 1 || total <= 1) {
        for (int64_t i = start; i < end; i++) {
            fn(i, ctx);
        }
        return;
    }

    tls_in_parallel_region = 1;

    if (num_w > total) num_w = (int)total;

    int64_t chunk = total / (num_w * 4);
    if (chunk < 1) chunk = 1;
    if (chunk > 2048) chunk = 2048;

    g_par_cur = start;
    g_par_end = end;
    g_par_chunk = chunk;
    g_par_fn = fn;
    g_par_ctx = ctx;

    for (int w = 1; w < num_w; w++) {
        g_worker_mode[w] = 1;
#ifdef _WIN32
        ResetEvent(g_done_events[w]);
        SetEvent(g_start_events[w]);
#else
        pthread_mutex_lock(&g_worker_mutexes[w]);
        g_worker_ready[w] = 1;
        g_worker_done[w] = 0;
        pthread_cond_signal(&g_worker_conds[w]);
        pthread_mutex_unlock(&g_worker_mutexes[w]);
#endif
    }

    // Current thread immediately executes dynamic chunks alongside workers
    datara_rt_run_dynamic_loop();

    // Wait for all worker threads to complete
#ifdef _WIN32
    if (num_w > 1) {
        WaitForMultipleObjects((DWORD)(num_w - 1), &g_done_events[1], TRUE, INFINITE);
    }
#else
    for (int w = 1; w < num_w; w++) {
        pthread_mutex_lock(&g_worker_mutexes[w]);
        while (!g_worker_done[w]) {
            pthread_cond_wait(&g_worker_conds[w], &g_worker_mutexes[w]);
        }
        pthread_mutex_unlock(&g_worker_mutexes[w]);
    }
#endif

    tls_in_parallel_region = 0;
}

void datara_rt_parallel_invoke(void (*fn1)(void* ctx1), void* ctx1, void (*fn2)(void* ctx2), void* ctx2) {
    datara_rt_ensure_threads();
    if (tls_in_parallel_region != 0 || g_workers_count <= 1) {
        if (fn1) fn1(ctx1);
        if (fn2) fn2(ctx2);
        return;
    }

    tls_in_parallel_region = 1;

    // Dispatch fn1 to worker 1
    g_worker_tasks[1].task_fn = fn1;
    g_worker_tasks[1].task_ctx = ctx1;
    g_worker_tasks[1].is_done = 0;
    g_worker_mode[1] = 2;

#ifdef _WIN32
    ResetEvent(g_done_events[1]);
    SetEvent(g_start_events[1]);
#else
    pthread_mutex_lock(&g_worker_mutexes[1]);
    g_worker_ready[1] = 1;
    g_worker_done[1] = 0;
    pthread_cond_signal(&g_worker_conds[1]);
    pthread_mutex_unlock(&g_worker_mutexes[1]);
#endif

    // Run fn2 on current thread concurrently
    if (fn2) fn2(ctx2);

    // Wait for worker 1
#ifdef _WIN32
    WaitForSingleObject(g_done_events[1], INFINITE);
#else
    pthread_mutex_lock(&g_worker_mutexes[1]);
    while (!g_worker_done[1]) {
        pthread_cond_wait(&g_worker_conds[1], &g_worker_mutexes[1]);
    }
    pthread_mutex_unlock(&g_worker_mutexes[1]);
#endif

    tls_in_parallel_region = 0;
}

// ---------------------------------------------------------------------------
// Effect-Driven Ephemeral Frame Arena
// ---------------------------------------------------------------------------
#define DATARA_ARENA_SIZE (2 * 1024 * 1024) // 2MB thread-local ephemeral bump arena
#if defined(_MSC_VER)
static __declspec(thread) char* g_datara_arena = NULL;
static __declspec(thread) int64_t g_datara_arena_top = 0;
#else
static __thread char* g_datara_arena = NULL;
static __thread int64_t g_datara_arena_top = 0;
#endif

void* datara_rt_arena_alloc(int64_t bytes) {
    if (bytes <= 0) return NULL;
    int64_t aligned = (bytes + 7) & ~7;
    if (!g_datara_arena) {
        g_datara_arena = (char*)malloc(DATARA_ARENA_SIZE);
        if (!g_datara_arena) return malloc((size_t)bytes);
    }
    if (g_datara_arena_top + aligned > DATARA_ARENA_SIZE) {
        return malloc((size_t)bytes);
    }
    void* ptr = (void*)&g_datara_arena[g_datara_arena_top];
    g_datara_arena_top += aligned;
    return ptr;
}

int64_t datara_rt_arena_checkpoint(void) {
    return g_datara_arena_top;
}

void datara_rt_arena_reset(int64_t saved_top) {
    if (saved_top >= 0 && saved_top <= g_datara_arena_top) {
        g_datara_arena_top = saved_top;
    }
}

int64_t datara_rt_arena_remaining(void) {
    return (int64_t)DATARA_ARENA_SIZE - g_datara_arena_top;
}
