// M10 NatVar: native fiber runtime (PLAN_V150 M10, slice 1).
//
// One-thread fiber scheduler: datara_rt_fiber_spawn registers a fiber whose
// body is a Datara function (address taken via GetFuncAddr, ABI
// void(*fn)(int64_t) — the parallel-invoke contract); the main thread
// converts itself to the scheduler fiber and round-robins READY fibers until
// every fiber is DONE (datara_rt_fiber_run_until_done).
//
// Fibers can yield (datara_rt_fiber_yield), park for a while
// (datara_rt_fiber_sleep_ms — timer wheel, non-blocking for other fibers)
// and wait for a peer (datara_rt_fiber_join). Channels:
// datara_rt_fiber_chan_create/send/recv — a blocking rendezvous queue built
// on the same parking machinery.
//
// Windows: CreateFiberEx / SwitchToFiber. POSIX: ucontext makecontext /
// swapcontext. Both provide cooperative stackful switches; no coloring, no
// state-machine boxing.

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
#include <ucontext.h>
#include <unistd.h>
#include <time.h>
#include <sys/time.h>
#endif

#include "datara_runtime.h"

static int64_t now_ms_real(void);

typedef enum {
    FIB_READY = 0,
    FIB_RUNNING,
    FIB_DONE,
    FIB_SLEEPING,
    FIB_WAITING_JOIN,
    FIB_WAITING_CHAN,
} FiberState;

typedef struct Fiber {
    int64_t handle;
    void* os_fiber;         // Windows LPVOID / POSIX ucontext_t*
    void (*fn)(int64_t);
    int64_t arg;
    FiberState state;
    int64_t wake_ms;        // valid when SLEEPING
    int64_t wait_on;        // fiber handle or channel id when WAITING_*
    int64_t exit_code;
    struct Fiber* next_ready;
#ifdef __linux__
    struct Fiber* next_free_ctx;
#endif
} Fiber;

#ifdef _WIN32
static void* g_scheduler_fiber = NULL;
static int g_converted = 0;
static unsigned long g_scheduler_thread = 0;
#else
static ucontext_t* g_scheduler_ctx = NULL;
#endif

// The scheduler fiber belongs to ONE thread (Windows ConvertThreadToFiber is
// thread-local). Test harnesses run each test on a fresh thread, so a stale
// converted state from a dead thread must be detected and reset before any
// SwitchToFiber — otherwise the new thread switches to a foreign fiber and
// crashes.
static void ensure_scheduler_fiber(void) {
#ifdef _WIN32
    unsigned long self_tid = GetCurrentThreadId();
    if (g_converted && g_scheduler_thread != self_tid) {
        datara_rt_fiber_reset();
    }
    if (!g_converted) {
        g_scheduler_fiber = ConvertThreadToFiber(NULL);
        g_converted = g_scheduler_fiber != NULL;
        g_scheduler_thread = self_tid;
    }
#else
    // POSIX ucontext state is not thread-bound; nothing to re-check.
#endif
}

#define FIBER_MAX 262144
static Fiber* g_fibers[FIBER_MAX];
static int64_t g_fiber_count = 0;
static int64_t g_next_handle = 1;

// Ready queue (FIFO, singly-linked).
static Fiber* g_ready_head = NULL;
static Fiber* g_ready_tail = NULL;
static Fiber* g_running = NULL;
static int64_t g_done_count = 0;

// Timer list (unordered; wake scan is O(n) — fine for the slice; the
// work-stealing multi-threaded path is the Rust chase_lev foundation).
#define TIMER_MAX 131072
static Fiber* g_sleeping[TIMER_MAX];
static int64_t g_sleeping_count = 0;

static int64_t now_ms_real(void) {
#ifdef _WIN32
    return (int64_t)GetTickCount64();
#else
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (int64_t)tv.tv_sec * 1000 + (int64_t)tv.tv_usec / 1000;
#endif
}

// ---------------------------------------------------------------- channels
typedef struct FiberChan {
    int64_t id;
    int64_t buf;            // single-slot rendezvous value
    Fiber* waiting_recv;    // fiber parked waiting for a value
    Fiber* waiting_send;    // fiber parked waiting for a receiver
    int has_value;
} FiberChan;

#define CHAN_MAX 65536
static FiberChan* g_chans[CHAN_MAX];
static int64_t g_chan_count = 0;

static Fiber* fiber_by_handle(int64_t h) {
    for (int64_t i = 0; i < g_fiber_count; i++) {
        if (g_fibers[i]->handle == h) return g_fibers[i];
    }
    return NULL;
}

static void ready_push(Fiber* f) {
    f->next_ready = NULL;
    if (g_ready_tail) {
        g_ready_tail->next_ready = f;
    } else {
        g_ready_head = f;
    }
    g_ready_tail = f;
}

static Fiber* ready_pop(void) {
    Fiber* f = g_ready_head;
    if (!f) return NULL;
    g_ready_head = f->next_ready;
    if (!g_ready_head) g_ready_tail = NULL;
    f->next_ready = NULL;
    return f;
}

static void timer_add(Fiber* f) {
    if (g_sleeping_count < TIMER_MAX) {
        g_sleeping[g_sleeping_count++] = f;
    }
}

static Fiber* timer_pop_due(int64_t now, int64_t* next_wake) {
    Fiber* due = NULL;
    *next_wake = -1;
    for (int64_t i = 0; i < g_sleeping_count; i++) {
        Fiber* f = g_sleeping[i];
        if (f->state != FIB_SLEEPING) continue;
        if (f->wake_ms <= now) {
            due = f;
            // remove by swap
            g_sleeping[i] = g_sleeping[g_sleeping_count - 1];
            g_sleeping_count--;
            i--;
            break;
        }
        if (*next_wake < 0 || f->wake_ms < *next_wake) *next_wake = f->wake_ms;
    }
    return due;
}

static Fiber* g_waiting_join[TIMER_MAX];
static int64_t g_waiting_join_count = 0;
static Fiber* g_waiting_chan_recv[TIMER_MAX];
static Fiber* g_waiting_chan_send[TIMER_MAX];
static int64_t g_waiting_chan_count = 0;

static void waiting_join_add(Fiber* f) {
    if (g_waiting_join_count < TIMER_MAX) {
        g_waiting_join[g_waiting_join_count++] = f;
    }
}

static void wake_waiters_of(int64_t target_handle) {
    for (int64_t i = 0; i < g_waiting_join_count; i++) {
        Fiber* f = g_waiting_join[i];
        if (f->state == FIB_WAITING_JOIN && f->wait_on == target_handle) {
            f->state = FIB_READY;
            ready_push(f);
            g_waiting_join[i] = g_waiting_join[g_waiting_join_count - 1];
            g_waiting_join_count--;
            i--;
        }
    }
}

static void wake_recv_of(int64_t chan_id) {
    for (int64_t i = 0; i < g_waiting_chan_count; i++) {
        Fiber* f = g_waiting_chan_recv[i];
        if (f && f->state == FIB_WAITING_CHAN && f->wait_on == chan_id) {
            f->state = FIB_READY;
            ready_push(f);
            g_waiting_chan_recv[i] = g_waiting_chan_recv[g_waiting_chan_count - 1];
            g_waiting_chan_count--;
            i--;
            break;
        }
    }
}

static void wake_send_of(int64_t chan_id) {
    for (int64_t i = 0; i < g_waiting_chan_count; i++) {
        Fiber* f = g_waiting_chan_send[i];
        if (f && f->state == FIB_WAITING_CHAN && f->wait_on == chan_id) {
            f->state = FIB_READY;
            ready_push(f);
            g_waiting_chan_send[i] = g_waiting_chan_send[g_waiting_chan_count - 1];
            g_waiting_chan_count--;
            i--;
            break;
        }
    }
}

// ------------------------------------------------------------- fiber entry
static void fiber_trampoline_entry(void) {
    Fiber* self = g_running;
    self->fn(self->arg);
    self->exit_code = 0;
    self->state = FIB_DONE;
    g_done_count++;
    (void)0;
}

// -------------------------------------------------------------- creation
#ifdef _WIN32
static void WINAPI win_tramp(void* param) {
    (void)param;
    fiber_trampoline_entry();
    // Return to scheduler when the body completes.
    SwitchToFiber(g_scheduler_fiber);
}
#else
static Fiber* g_posix_self = NULL;
static void posix_tramp(uint32_t lo, uint32_t hi) {
    uint64_t packed = ((uint64_t)hi << 32) | (uint64_t)lo;
    g_posix_self = (Fiber*)packed;
    fiber_trampoline_entry();
    // Return to the scheduler
    swapcontext((ucontext_t*)g_posix_self->os_fiber, g_scheduler_ctx);
}
#endif

int64_t datara_rt_fiber_spawn(int64_t fn_addr, int64_t arg, int64_t stack_kb) {
    if (g_fiber_count >= FIBER_MAX) return -1;
    // ALWAYS ensure the calling thread owns the scheduler fiber: the check
    // detects a stale converted state left by a dead thread (test harnesses
    // run each program on a fresh thread — the g_next_handle==1 guard used
    // to skip the check for every program after the first).
    ensure_scheduler_fiber();
#ifndef _WIN32
    if (!g_scheduler_ctx) {
        g_scheduler_ctx = (ucontext_t*)malloc(sizeof(ucontext_t));
        getcontext(g_scheduler_ctx);
    }
#endif
    Fiber* f = (Fiber*)calloc(1, sizeof(Fiber));
    if (!f) return -1;
    f->handle = g_next_handle++;
    f->fn = (void (*)(int64_t))(uintptr_t)fn_addr;
    f->arg = arg;
    f->state = FIB_READY;

    size_t stack_bytes = (size_t)(stack_kb > 0 ? stack_kb : 8) * 1024;

#ifdef _WIN32
    f->os_fiber = CreateFiberEx(16 * 1024, stack_bytes, 0, win_tramp, NULL);
    if (!f->os_fiber) { free(f); return -1; }
#else
    ucontext_t* uc = (ucontext_t*)malloc(sizeof(ucontext_t));
    if (!uc) { free(f); return -1; }
    getcontext(uc);
    uc->uc_stack.ss_sp = malloc(stack_bytes);
    if (!uc->uc_stack.ss_sp) { free(uc); free(f); return -1; }
    uc->uc_stack.ss_size = stack_bytes;
    uc->uc_link = NULL;
    uint64_t packed = (uint64_t)(uintptr_t)f;
    makecontext(uc, (void (*)(void))posix_tramp, 2,
                (uint32_t)(packed & 0xFFFFFFFFULL),
                (uint32_t)(packed >> 32));
    f->os_fiber = uc;
#endif

    g_fibers[g_fiber_count++] = f;
    ready_push(f);
    return f->handle;
}

int64_t datara_rt_fiber_count(void) {
    return g_fiber_count;
}

int64_t datara_rt_fiber_done_count(void) {
    return g_done_count;
}

static void fiber_switch_to(Fiber* f) {
    g_running = f;
    f->state = FIB_RUNNING;
#ifdef _WIN32
    SwitchToFiber(f->os_fiber);
#else
    swapcontext(g_scheduler_ctx, (ucontext_t*)f->os_fiber);
#endif
    g_running = NULL;
}

void datara_rt_fiber_yield(void) {
    Fiber* self = g_running;
    if (!self) return; // called outside a fiber — no-op
    self->state = FIB_READY;
    ready_push(self);
#ifdef _WIN32
    SwitchToFiber(g_scheduler_fiber);
#else
    swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
}

void datara_rt_fiber_sleep_ms(int64_t ms) {
    Fiber* self = g_running;
    if (!self) return;
    self->state = FIB_SLEEPING;
    self->wake_ms = now_ms_real() + ms;
    timer_add(self);
#ifdef _WIN32
    SwitchToFiber(g_scheduler_fiber);
#else
    swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
}

int64_t datara_rt_fiber_join(int64_t handle) {
    Fiber* self = g_running;
    if (!self) {
        // Join from the scheduler fiber (main): only legal when done.
        Fiber* t = fiber_by_handle(handle);
        return (t && t->state == FIB_DONE) ? t->exit_code : -1;
    }
    Fiber* target = fiber_by_handle(handle);
    if (!target) return -1;
    while (target->state != FIB_DONE) {
        self->state = FIB_WAITING_JOIN;
        self->wait_on = handle;
        waiting_join_add(self);
#ifdef _WIN32
        SwitchToFiber(g_scheduler_fiber);
#else
        swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
    }
    return target->exit_code;
}

// Main-loop scheduler: run until every spawned fiber is DONE. Returns the
// number of fibers completed.
int64_t datara_rt_fiber_run_until_done(void) {
    if (g_fiber_count == 0) return 0;
    ensure_scheduler_fiber();
#ifndef _WIN32
    if (!g_scheduler_ctx) {
        g_scheduler_ctx = (ucontext_t*)malloc(sizeof(ucontext_t));
        getcontext(g_scheduler_ctx);
    }
#endif
#ifdef _WIN32
    if (!g_converted) return -1;
#endif
    int64_t last_now = now_ms_real();
    for (;;) {
        Fiber* f = ready_pop();
        if (!f) {
            f = timer_pop_due(last_now, &last_now);
        }
        if (!f) {
            if (g_done_count >= g_fiber_count) break;
            // Nothing ready: if timers exist, sleep to the nearest wake;
            // otherwise a join-deadlock — break with an error code.
            int64_t next_wake = -1;
            (void)timer_pop_due(now_ms_real(), &next_wake);
            if (next_wake >= 0) {
#ifdef _WIN32
                Sleep((DWORD)(next_wake > last_now ? next_wake - last_now : 0));
#else
                usleep((useconds_t)(next_wake > last_now ? (next_wake - last_now) * 1000 : 0));
#endif
                last_now = now_ms_real();
                continue;
            }
            // True deadlock (join on a never-finishing fiber).
            break;
        }
        fiber_switch_to(f);
        last_now = now_ms_real();
    }
    return g_done_count;
}

void datara_rt_fiber_reset(void) {
    for (int64_t i = 0; i < g_fiber_count; i++) {
        Fiber* f = g_fibers[i];
#ifdef _WIN32
        if (f->os_fiber) DeleteFiber(f->os_fiber);
#else
        ucontext_t* uc = (ucontext_t*)f->os_fiber;
        if (uc) {
            free(uc->uc_stack.ss_sp);
            free(uc);
        }
#endif
        free(f);
    }
    g_fiber_count = 0;
    g_next_handle = 1;
    g_ready_head = g_ready_tail = NULL;
    g_running = NULL;
    g_done_count = 0;
    g_sleeping_count = 0;
    g_waiting_join_count = 0;
    g_waiting_chan_count = 0;
    for (int64_t i = 0; i < g_chan_count; i++) free(g_chans[i]);
    g_chan_count = 0;
#ifdef _WIN32
    if (g_converted) {
        // ConvertFiberToThread is only legal on the thread that converted;
        // a reset from a different thread (dead owner) must skip it.
        if (GetCurrentThreadId() == g_scheduler_thread) {
            ConvertFiberToThread();
        }
        g_converted = 0;
        g_scheduler_fiber = NULL;
        g_scheduler_thread = 0;
    }
#else
    if (g_scheduler_ctx) {
        free(g_scheduler_ctx);
        g_scheduler_ctx = NULL;
    }
#endif
}

// ---------------------------------------------------------------- channels
int64_t datara_rt_fiber_chan_create(void) {
    if (g_chan_count >= CHAN_MAX) return -1;
    FiberChan* c = (FiberChan*)calloc(1, sizeof(FiberChan));
    if (!c) return -1;
    c->id = g_chan_count + 1;
    g_chans[g_chan_count++] = c;
    return c->id;
}

static FiberChan* chan_by_id(int64_t id) {
    if (id < 1 || id > g_chan_count) return NULL;
    return g_chans[id - 1];
}

void datara_rt_fiber_chan_send(int64_t chan_id, int64_t value) {
    Fiber* self = g_running;
    FiberChan* c = chan_by_id(chan_id);
    if (!c || !self) return;
    c->buf = value;
    c->has_value = 1;
    // Wake a parked receiver.
    wake_recv_of(chan_id);
    if (c->has_value) {
        // Rendezvous: park the sender until the value is consumed.
        self->state = FIB_WAITING_CHAN;
        self->wait_on = chan_id;
        if (g_waiting_chan_count < TIMER_MAX) {
            g_waiting_chan_send[g_waiting_chan_count++] = self;
        }
#ifdef _WIN32
        SwitchToFiber(g_scheduler_fiber);
#else
        swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
    }
}

int64_t datara_rt_fiber_chan_recv(int64_t chan_id) {
    Fiber* self = g_running;
    FiberChan* c = chan_by_id(chan_id);
    if (!c) return -1;
    for (;;) {
        if (c->has_value) {
            int64_t v = c->buf;
            c->has_value = 0;
            wake_send_of(chan_id);
            return v;
        }
        if (!self) {
            // Called from the scheduler (main): step the scheduler until a
            // sender appears — a fiber↔thread rendezvous. If no work and no
            // sleeping fibers and no senders remain, fail closed.
            Fiber* f = ready_pop();
            int64_t next_wake = -1;
            if (!f) {
                f = timer_pop_due(now_ms_real(), &next_wake);
            }
            if (!f) {
                if (g_done_count >= g_fiber_count) return -1;
                if (next_wake >= 0) {
#ifdef _WIN32
                    Sleep((DWORD)(next_wake > now_ms_real() ? next_wake - now_ms_real() : 0));
#else
                    usleep((useconds_t)1000);
#endif
                }
                continue;
            }
            fiber_switch_to(f);
            continue;
        }
        self->state = FIB_WAITING_CHAN;
        self->wait_on = chan_id;
        if (g_waiting_chan_count < TIMER_MAX) {
            g_waiting_chan_recv[g_waiting_chan_count++] = self;
        }
#ifdef _WIN32
        SwitchToFiber(g_scheduler_fiber);
#else
        swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
    }
}

// --------------------------------------------------------------- test util
// Fiber-friendly monotonic clock for benchmarks.
int64_t datara_rt_fiber_now_ms(void) {
    return now_ms_real();
}
