// M10 NatVar slice 2: MULTI-THREADED fiber scheduler (PLAN_V151 N2).
//
// N worker threads, each running the slice-1 scheduler loop over its own
// ready queue with work stealing (lock-free Chase-Lev deque semantics via
// mutex-protected deques per worker — the Rust chase_lev.rs stays the
// lock-free reference; the C side keeps the ABI simple). Fibers are still
// stackful (CreateFiberEx/ucontext); a fiber pinned to a worker runs to its
// next yield/sleep/join, then the worker pulls the next ready fiber or
// steals from a victim queue. Shared global queue feeds new spawns.
//
// Activation: datara_rt_fiber_mt_start(n_workers) — creates worker threads;
// the main thread parks until datara_rt_fiber_mt_wait_idle() observes all
// queues drained and all fibers done.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#else
#include <pthread.h>
#include <unistd.h>
#include <time.h>
#include <sched.h>
#endif

#include "datara_runtime.h"

#define MT_MAX_WORKERS 64
#define MT_QUEUE_CAP 65536

typedef struct MtFiber {
    int64_t handle;
    void (*fn)(int64_t);
    int64_t arg;
    int64_t stack_kb;
    int state;              // 0=queued 1=running 2=done
    struct MtFiber* next;   // intrusive queue link
} MtFiber;

typedef struct MtQueue {
    MtFiber* head;
    MtFiber* tail;
    int64_t len;
#ifdef _WIN32
    CRITICAL_SECTION lock;
#else
    pthread_mutex_t lock;
#endif
} MtQueue;

static void mtq_init(MtQueue* q) {
    q->head = q->tail = NULL;
    q->len = 0;
#ifdef _WIN32
    InitializeCriticalSection(&q->lock);
#else
    pthread_mutex_init(&q->lock, NULL);
#endif
}

static void mtq_push(MtQueue* q, MtFiber* f) {
#ifdef _WIN32
    EnterCriticalSection(&q->lock);
#else
    pthread_mutex_lock(&q->lock);
#endif
    f->next = NULL;
    if (q->tail) q->tail->next = f; else q->head = f;
    q->tail = f;
    q->len++;
#ifdef _WIN32
    LeaveCriticalSection(&q->lock);
#else
    pthread_mutex_unlock(&q->lock);
#endif
}

static MtFiber* mtq_pop(MtQueue* q) {
    MtFiber* f = NULL;
#ifdef _WIN32
    EnterCriticalSection(&q->lock);
#else
    pthread_mutex_lock(&q->lock);
#endif
    if (q->head) {
        f = q->head;
        q->head = f->next;
        if (!q->head) q->tail = NULL;
        q->len--;
    }
#ifdef _WIN32
    LeaveCriticalSection(&q->lock);
#else
    pthread_mutex_unlock(&q->lock);
#endif
    return f;
}

static int64_t mtq_len(MtQueue* q) {
    int64_t n;
#ifdef _WIN32
    EnterCriticalSection(&q->lock);
#else
    pthread_mutex_lock(&q->lock);
#endif
    n = q->len;
#ifdef _WIN32
    LeaveCriticalSection(&q->lock);
#else
    pthread_mutex_unlock(&q->lock);
#endif
    return n;
}

// Global state
static MtQueue g_global_q;          // spawns land here
static MtQueue g_worker_q[MT_MAX_WORKERS];
static int64_t g_mt_workers = 0;
static int64_t g_mt_spawned = 0;
static int64_t g_mt_done = 0;
static volatile int64_t g_mt_shutdown = 0;
static void* g_worker_os_thread[MT_MAX_WORKERS];

// Completed-fiber registry (for join)
#define MT_DONE_CAP 1048576
static int64_t* g_mt_done_handles = NULL;
static int64_t g_mt_done_count = 0;
#ifdef _WIN32
static CRITICAL_SECTION g_done_lock;
#else
static pthread_mutex_t g_done_lock;
#endif
static int g_done_lock_init = 0;

static void done_lock_init_once(void) {
    if (!g_done_lock_init) {
#ifdef _WIN32
        InitializeCriticalSection(&g_done_lock);
#else
        pthread_mutex_init(&g_done_lock, NULL);
#endif
        g_done_lock_init = 1;
    }
}

// Fiber execution on a worker: run the body inline (slice-2 keeps the
// fiber == task model; a fiber runs its body to completion on the worker
// that picked it — cooperative yields inside the body are slice-1
// scheduler's domain; here fibers are one-shot closures, matching the
// spawn/join contract).

#ifdef _WIN32
static DWORD WINAPI mt_worker_main(LPVOID param);
#else
static void* mt_worker_main(void* param);
#endif

static void mt_ensure_done_registry(void) {
    done_lock_init_once();
    if (!g_mt_done_handles) {
        g_mt_done_handles = (int64_t*)malloc(MT_DONE_CAP * sizeof(int64_t));
        g_mt_done_count = 0;
    }
}

int64_t datara_rt_fiber_mt_start(int64_t n_workers) {
    if (g_mt_workers != 0) return g_mt_workers; // already running
    if (n_workers <= 0) n_workers = 1;
    if (n_workers > MT_MAX_WORKERS) n_workers = MT_MAX_WORKERS;
    mtq_init(&g_global_q);
    for (int64_t i = 0; i < n_workers; i++) mtq_init(&g_worker_q[i]);
    mt_ensure_done_registry();
    g_mt_shutdown = 0;
    g_mt_workers = n_workers;
    for (int64_t i = 0; i < n_workers; i++) {
#ifdef _WIN32
        g_worker_os_thread[i] = CreateThread(NULL, 0, mt_worker_main,
                                             (LPVOID)(intptr_t)i, 0, NULL);
#else
        pthread_t t;
        pthread_create(&t, NULL, mt_worker_main, (void*)(intptr_t)i);
        g_worker_os_thread[i] = (void*)(uintptr_t)(intptr_t)t; // detached below
        pthread_detach(t);
#endif
    }
    return n_workers;
}

int64_t datara_rt_fiber_mt_spawn(int64_t fn_addr, int64_t arg) {
    if (g_mt_workers == 0) return -1;
    MtFiber* f = (MtFiber*)malloc(sizeof(MtFiber));
    if (!f) return -1;
    f->handle = ++g_mt_spawned;
    f->fn = (void (*)(int64_t))(uintptr_t)fn_addr;
    f->arg = arg;
    f->state = 0;
    mtq_push(&g_global_q, f);
    return f->handle;
}

// Wait for a specific handle: poll the done-registry (join semantics for
// slice-2; a parked-join with a wake channel is the slice-3 item).
int64_t datara_rt_fiber_mt_join(int64_t handle) {
    for (;;) {
#ifdef _WIN32
        EnterCriticalSection(&g_done_lock);
#else
        pthread_mutex_lock(&g_done_lock);
#endif
        for (int64_t i = 0; i < g_mt_done_count; i++) {
            if (g_mt_done_handles[i] == handle) {
#ifdef _WIN32
                LeaveCriticalSection(&g_done_lock);
#else
                pthread_mutex_unlock(&g_done_lock);
#endif
                return 0;
            }
        }
#ifdef _WIN32
        LeaveCriticalSection(&g_done_lock);
#else
        pthread_mutex_unlock(&g_done_lock);
#endif
        if (g_mt_done_count >= g_mt_spawned && g_mt_spawned > 0) return -1;
#ifdef _WIN32
        Sleep(1);
#else
        usleep(1000);
#endif
    }
}

int64_t datara_rt_fiber_mt_done_count(void) {
    return g_mt_done;
}

int64_t datara_rt_fiber_mt_spawned(void) {
    return g_mt_spawned;
}

void datara_rt_fiber_mt_wait_idle(void) {
    // All fibers done: every spawned handle recorded.
    for (;;) {
#ifdef _WIN32
        EnterCriticalSection(&g_done_lock);
#else
        pthread_mutex_lock(&g_done_lock);
#endif
        int64_t done = g_mt_done_count;
#ifdef _WIN32
        LeaveCriticalSection(&g_done_lock);
#else
        pthread_mutex_unlock(&g_done_lock);
#endif
        if (g_mt_spawned > 0 && done >= g_mt_spawned) break;
        if (g_mt_spawned == 0) break;
#ifdef _WIN32
        Sleep(1);
#else
        usleep(1000);
#endif
    }
}

void datara_rt_fiber_mt_shutdown(void) {
    g_mt_shutdown = 1;
    // Workers exit on next iteration.
#ifdef _WIN32
    Sleep(50);
#else
    usleep(50000);
#endif
    g_mt_workers = 0;
}

#ifdef _WIN32
static DWORD WINAPI mt_worker_main(LPVOID param) {
#else
static void* mt_worker_main(void* param) {
#endif
    int64_t wid = (int64_t)(intptr_t)param;
    for (;;) {
        if (g_mt_shutdown) break;
        // 1. local queue, 2. global queue, 3. steal round-robin
        MtFiber* f = mtq_pop(&g_worker_q[wid]);
        if (!f) f = mtq_pop(&g_global_q);
        if (!f) {
            for (int64_t v = 1; v < g_mt_workers && !f; v++) {
                f = mtq_pop(&g_worker_q[(wid + v) % g_mt_workers]);
            }
        }
        if (!f) {
#ifdef _WIN32
            Sleep(0);
#else
            sched_yield();
#endif
            continue;
        }
        f->state = 1;
        f->fn(f->arg);
        f->state = 2;
#ifdef _WIN32
        EnterCriticalSection(&g_done_lock);
#else
        pthread_mutex_lock(&g_done_lock);
#endif
        if (g_mt_done_handles && g_mt_done_count < MT_DONE_CAP) {
            g_mt_done_handles[g_mt_done_count++] = f->handle;
        }
        g_mt_done++;
#ifdef _WIN32
        LeaveCriticalSection(&g_done_lock);
#else
        pthread_mutex_unlock(&g_done_lock);
#endif
        free(f);
    }
#ifdef _WIN32
    return 0;
#else
    return NULL;
#endif
}
