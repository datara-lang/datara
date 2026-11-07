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
  #include <winsock2.h>
  #include <ws2tcpip.h>
#else
  #include <sys/socket.h>
  #include <sys/select.h>
  #include <unistd.h>
  #include <errno.h>
  #include <sys/time.h>
#endif

// macOS: ucontext routines are deprecated XSI extensions — without
// _XOPEN_SOURCE the SDK header emits #error and makecontext/swapcontext
// stay undeclared (CI: Test (macos-latest) failure).
#if defined(__APPLE__) && !defined(_XOPEN_SOURCE)
#define _XOPEN_SOURCE 600
#endif

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
extern void* datara_rt_outcome_build(int64_t is_success, const char* value, const char* msg);

typedef enum {
    FIB_READY = 0,
    FIB_RUNNING,
    FIB_DONE,
    FIB_SLEEPING,
    FIB_WAITING_JOIN,
    FIB_WAITING_CHAN,
    FIB_WAITING_IO,
    FIB_WAITING_MUTEX,
} FiberState;

typedef struct Fiber {
    int64_t handle;
    void* os_fiber;         // Windows LPVOID / POSIX ucontext_t*
    void (*fn)(int64_t);
    int64_t arg;
    FiberState state;
    int64_t wake_ms;        // valid when SLEEPING
    int64_t wait_on;        // fiber handle or channel id when WAITING_*
    int64_t wait_fd;        // socket fd when WAITING_IO
    int64_t wait_deadline;  // absolute ms deadline when WAITING_IO (0 = inf)
    int64_t wake_result;    // why the waker resumed us: 1 = granted, 0 = timeout
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
    // Wake the EARLIEST-due fiber (minimum wake_ms), not the first scanned
    // one: two fibers with different sleep deadlines must wake in deadline
    // order even when both are already due during one scan (loaded CI
    // runners oversleep both).
    Fiber* due = NULL;
    int64_t due_idx = -1;
    *next_wake = -1;
    for (int64_t i = 0; i < g_sleeping_count; i++) {
        Fiber* f = g_sleeping[i];
        if (f->state != FIB_SLEEPING) continue;
        if (f->wake_ms <= now) {
            if (!due || f->wake_ms < due->wake_ms) {
                due = f;
                due_idx = i;
            }
        } else if (*next_wake < 0 || f->wake_ms < *next_wake) {
            *next_wake = f->wake_ms;
        }
    }
    if (due) {
        g_sleeping[due_idx] = g_sleeping[g_sleeping_count - 1];
        g_sleeping_count--;
    }
    return due;
}

// Scan-only: the earliest future wake time among sleeping fibers (never
// extracts). Used by the drain loop to decide whether waiting would help.
static int64_t timer_peek_next_wake(void) {
    int64_t next_wake = -1;
    for (int64_t i = 0; i < g_sleeping_count; i++) {
        Fiber* f = g_sleeping[i];
        if (f->state != FIB_SLEEPING) continue;
        if (next_wake < 0 || f->wake_ms < next_wake) next_wake = f->wake_ms;
    }
    return next_wake;
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


// ---- IO wait list (PLAN_V152 N1.1): fibers parked on socket readability.
#define NATVAR_IO_MAX 256
static Fiber* g_io_waiters[NATVAR_IO_MAX];
static int64_t g_io_waiter_count = 0;

static void io_wait_add(Fiber* f) {
    if (g_io_waiter_count < NATVAR_IO_MAX) {
        g_io_waiters[g_io_waiter_count++] = f;
    }
}

static int io_wait_poll_and_wake(void) {
    // Poll all parked fds via select(); wake readable ones (push to ready).
    // Returns the number woken.
    if (g_io_waiter_count == 0) return 0;
#ifdef _WIN32
    fd_set rfds;
    FD_ZERO(&rfds);
    SOCKET maxfd = 0;
    for (int64_t i = 0; i < g_io_waiter_count; i++) {
        SOCKET fd = (SOCKET)g_io_waiters[i]->wait_fd;
        FD_SET(fd, &rfds);
        if (fd > maxfd) maxfd = fd;
    }
    struct timeval tv = { 0, 2000 }; // 2 ms slice
    int nready = select((int)maxfd + 1, &rfds, NULL, NULL, &tv);
    if (nready <= 0) return 0;
    int64_t woken = 0;
    int64_t w = 0;
    for (int64_t i = 0; i < g_io_waiter_count; i++) {
        Fiber* f = g_io_waiters[i];
        SOCKET fd = (SOCKET)f->wait_fd;
        int64_t expired = f->wait_deadline > 0 && now_ms_real() >= f->wait_deadline;
        if (FD_ISSET(fd, &rfds) || expired) {
            f->state = FIB_READY;
            ready_push(f);
            woken++;
        } else {
            g_io_waiters[w++] = f;
        }
    }
    g_io_waiter_count = w;
    return (int)woken;
#else
    int maxfd = 0;
    fd_set rfds;
    FD_ZERO(&rfds);
    for (int64_t i = 0; i < g_io_waiter_count; i++) {
        int fd = (int)g_io_waiters[i]->wait_fd;
        FD_SET(fd, &rfds);
        if (fd > maxfd) maxfd = fd;
    }
    struct timeval tv = { 0, 2000 };
    int nready = select(maxfd + 1, &rfds, NULL, NULL, &tv);
    if (nready <= 0) return 0;
    int64_t woken = 0;
    int64_t w = 0;
    for (int64_t i = 0; i < g_io_waiter_count; i++) {
        Fiber* f = g_io_waiters[i];
        int fd = (int)f->wait_fd;
        int64_t expired = f->wait_deadline > 0 && now_ms_real() >= f->wait_deadline;
        if (FD_ISSET(fd, &rfds) || expired) {
            f->state = FIB_READY;
            ready_push(f);
            woken++;
        } else {
            g_io_waiters[w++] = f;
        }
    }
    g_io_waiter_count = w;
    return (int)woken;
#endif
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

// PLAN_V152 N1.3: non-blocking completion poll (FiberHandle.is_done).
int64_t datara_rt_fiber_is_done(int64_t handle) {
    Fiber* t = fiber_by_handle(handle);
    return (t && t->state == FIB_DONE) ? 1 : 0;
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

// ============================================================================
// Fiber-aware non-blocking ACCEPT (PLAN_V153 A2). The plain accept() call
// BLOCKS the OS thread — on the single-threaded fiber scheduler that froze
// every other fiber (serve-before-connect = total deadlock). This variant
// flips the listener to non-blocking, tries accept, and parks the fiber on
// would-block; the scheduler's select() slice wakes it when a connection
// arrives.
// ============================================================================
void* datara_rt_fiber_net_accept(int64_t sock, int64_t timeout_ms) {
    if (sock < 0) return datara_rt_outcome_build(0, "", "invalid listener");
    Fiber* self = g_running;
    for (;;) {
#ifdef _WIN32
        SOCKET client = accept((SOCKET)sock, NULL, NULL);
        if (client != INVALID_SOCKET) {
            char handle_buf[32];
            snprintf(handle_buf, sizeof handle_buf, "%lld", (long long)client);
            return datara_rt_outcome_build(1, handle_buf, "");
        }
        int err = WSAGetLastError();
        if (err != WSAEWOULDBLOCK && err != WSAEINPROGRESS) {
            return datara_rt_outcome_build(0, "", "accept error");
        }
#else
        int client = accept((int)sock, NULL, NULL);
        if (client >= 0) {
            char handle_buf[32];
            snprintf(handle_buf, sizeof handle_buf, "%d", client);
            return datara_rt_outcome_build(1, handle_buf, "");
        }
        if (errno != EWOULDBLOCK && errno != EAGAIN && errno != EINPROGRESS) {
            return datara_rt_outcome_build(0, "", "accept error");
        }
#endif
        if (!self) {
#ifdef _WIN32
            Sleep(10);
#else
            usleep(10000);
#endif
            continue;
        }
        self->state = FIB_WAITING_IO;
        self->wait_fd = sock;
        self->wait_deadline = timeout_ms > 0 ? now_ms_real() + timeout_ms : 0;
        io_wait_add(self);
#ifdef _WIN32
        SwitchToFiber(g_scheduler_fiber);
#else
        swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
    }
}

// Main-loop scheduler: run until every spawned fiber is DONE. Returns the
// number of fibers completed.

// ============================================================================
// Fiber-aware cooperative mutexes (PLAN_V152 N1.2). A lock that cannot be
// granted immediately parks the calling fiber (FIB_WAITING_MUTEX); unlock
// grants ownership DIRECTLY to the first waiter (no thundering herd).
// datara_rt_mutex_lock returns an Outcome<Str>-shaped object:
//   ok  -> datara_rt_outcome_build(1, "locked", "")
//   err -> datara_rt_outcome_build(0, "", "timeout")   (deadline expired)
// The AB-BA scenario therefore cannot hang the scheduler: both fibers get
// an err outcome after their timeouts, and the deadlock detector (below)
// resolves FIB_WAITING_MUTEX edges to the owner fiber for cycle reports.
// ============================================================================
#define NATVAR_MUTEX_MAX 64
#define NATVAR_MUTEX_WAITERS_MAX 256
typedef struct {
    int64_t locked;
    int64_t owner; // fiber handle
    Fiber* waiters[NATVAR_MUTEX_WAITERS_MAX];
    int64_t wait_count;
} FiberMutex;
static FiberMutex g_mutexes[NATVAR_MUTEX_MAX];
static int64_t g_mutex_count = 0;

int64_t datara_rt_mutex_new(void) {
    if (g_mutex_count >= NATVAR_MUTEX_MAX) return -1;
    int64_t id = g_mutex_count++;
    g_mutexes[id].locked = 0;
    g_mutexes[id].owner = 0;
    g_mutexes[id].wait_count = 0;
    return id;
}

void* datara_rt_mutex_lock(int64_t id, int64_t timeout_ms) {
    if (id < 0 || id >= g_mutex_count) {
        return datara_rt_outcome_build(0, "", "invalid mutex handle");
    }
    FiberMutex* m = &g_mutexes[id];
    if (!m->locked) {
        m->locked = 1;
        Fiber* self = g_running;
        m->owner = self ? self->handle : 0;
        return datara_rt_outcome_build(1, "locked", "");
    }
    Fiber* self = g_running;
    if (!self || m->wait_count >= NATVAR_MUTEX_WAITERS_MAX) {
        // Outside a fiber (main) or wait-list full: fail loudly, never block.
        return datara_rt_outcome_build(0, "", "mutex busy");
    }
    self->state = FIB_WAITING_MUTEX;
    self->wait_on = id;
    self->wait_deadline = timeout_ms > 0 ? now_ms_real() + timeout_ms : 0;
    self->wake_result = -1;
    m->waiters[m->wait_count++] = self;
#ifdef _WIN32
    SwitchToFiber(g_scheduler_fiber);
#else
    swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
    // Resumed: the waker set wake_result (1 = granted, 0 = timeout).
    if (self->wake_result == 1) {
        return datara_rt_outcome_build(1, "locked", "");
    }
    return datara_rt_outcome_build(0, "", "timeout");
}

void datara_rt_mutex_unlock(int64_t id) {
    if (id < 0 || id >= g_mutex_count) return;
    FiberMutex* m = &g_mutexes[id];
    if (m->wait_count > 0) {
        // Grant ownership directly to the head waiter.
        Fiber* next = m->waiters[0];
        for (int64_t i = 1; i < m->wait_count; i++) {
            m->waiters[i - 1] = m->waiters[i];
        }
        m->wait_count--;
        m->owner = next->handle;
        next->wake_result = 1;
        next->state = FIB_READY;
        ready_push(next);
    } else {
        m->locked = 0;
        m->owner = 0;
    }
}

// Deadline scan for parked mutex waiters (called each scheduler pass).
static int mutex_wait_poll_and_wake(void) {
    int64_t woken = 0;
    for (int64_t id = 0; id < g_mutex_count; id++) {
        FiberMutex* m = &g_mutexes[id];
        int64_t w = 0;
        for (int64_t i = 0; i < m->wait_count; i++) {
            Fiber* f = m->waiters[i];
            int64_t expired =
                f->wait_deadline > 0 && now_ms_real() >= f->wait_deadline;
            if (expired) {
                f->wake_result = 0;
                f->state = FIB_READY;
                ready_push(f);
                woken++;
            } else {
                m->waiters[w++] = f;
            }
        }
        m->wait_count = w;
    }
    return (int)woken;
}

// ---- Deadlock detector (PLAN_V152 N1): cycle detection in the fiber
// wait graph. Called by run_until_done when it would otherwise break
// with a deadlock. Returns the first cycle path or NULL.

// Find the fiber a given fiber is waiting on (join or chan counterpart).
static Fiber* dd_resolve_wait_target(Fiber* f) {
    if (f->state == FIB_WAITING_JOIN) {
        return fiber_by_handle(f->wait_on);
    }
    if (f->state == FIB_WAITING_MUTEX) {
        // Mutex edge: the fiber waits for the CURRENT OWNER to unlock.
        if (f->wait_on >= 0 && f->wait_on < g_mutex_count) {
            int64_t owner = g_mutexes[f->wait_on].owner;
            if (owner > 0) {
                Fiber* of = fiber_by_handle(owner);
                if (of && of != f && of->state != FIB_DONE) return of;
            }
        }
        return NULL;
    }
    if (f->state == FIB_WAITING_CHAN) {
        // Channel rendezvous: find the peer on the same channel.
        for (int64_t i = 0; i < g_fiber_count; i++) {
            Fiber* peer = g_fibers[i];
            if (peer != f && peer->state == FIB_WAITING_CHAN && peer->wait_on == f->wait_on) {
                return peer;
            }
        }
    }
    return NULL;
}

// Detect cycle: follow wait edges up to g_fiber_count steps.
// If we revisit a fiber, there is a cycle. Returns 1 if cycle found,
// prints the path to stderr.
static int datara_deadlock_detect(void) {
    for (int64_t start = 0; start < g_fiber_count; start++) {
        Fiber* cur = g_fibers[start];
        if (cur->state != FIB_WAITING_JOIN && cur->state != FIB_WAITING_CHAN
            && cur->state != FIB_WAITING_MUTEX) continue;
        // Walk the chain
        int64_t visited[256];
        int64_t depth = 0;
        Fiber* node = cur;
        while (node && depth < 256) {
            for (int64_t v = 0; v < depth; v++) {
                if (visited[v] == node->handle) {
                    // Cycle!
                    fprintf(stderr, "[DEADLOCK] cycle detected:");
                    for (int64_t p = v; p < depth; p++) {
                        fprintf(stderr, " fib_%lld ->", (long long)visited[p]);
                    }
                    fprintf(stderr, " fib_%lld (back to fib_%lld)\n",
                            (long long)node->handle, (long long)visited[v]);
                    return 1;
                }
            }
            visited[depth++] = node->handle;
            node = dd_resolve_wait_target(node);
        }
    }
    return 0;
}

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
    int64_t dbg_iters = 0;
    for (;;) {
        // Fairness: promote due sleepers even when the ready queue is
        // non-empty — a continuously-yielding fiber used to starve them
        // forever, because the timer was consulted only when the queue
        // emptied (repro: a poller fiber doing yield+is_done() around a
        // sleeping worker hung the scheduler). The promoted fiber is
        // pushed to the READY queue (never dropped, never run inline) and
        // next_wake goes to a SCRATCH variable — passing &last_now let the
        // out-param overwrite the clock with a future wake time / -1.
        {
            int64_t fairness_next_wake = -1;
            Fiber* due = timer_pop_due(last_now, &fairness_next_wake);
            if (due) {
                due->state = FIB_READY;
                ready_push(due);
            }
        }
        if (getenv("DATARA_SCHED_TRACE") && (++dbg_iters % 1000000) == 0) {
            fprintf(stderr, "[sched] iters=%lld last_now=%lld done=%lld/%lld\n",
                    (long long)dbg_iters, (long long)last_now,
                    (long long)g_done_count, (long long)g_fiber_count);
            for (int64_t i = 0; i < g_fiber_count; i++) {
                fprintf(stderr, "[sched]   fib_%lld state=%d wake=%lld\n",
                        (long long)g_fibers[i]->handle, (int)g_fibers[i]->state,
                        (long long)g_fibers[i]->wake_ms);
            }
        }
        Fiber* f = ready_pop();
        if (!f) {
            f = timer_pop_due(last_now, &last_now);
        }
        if (!f) {
            if (g_done_count >= g_fiber_count) break;
            // Nothing ready: if timers exist, sleep to the nearest wake;
            // otherwise a join-deadlock — break with an error code.
            // CRITICAL: peek WITHOUT extraction — the previous
            // (void)timer_pop_due(...) call here EXTRACTED a fiber that had
            // become due during the Sleep and discarded it (the fiber was
            // removed from the timer wheel but never scheduled — a loaded
            // CI runner lost ticks: 3-sleep loop printed "2").
            // IO waiters: poll their fds; wake readable ones.
            if (g_io_waiter_count > 0) {
                io_wait_poll_and_wake();
                last_now = now_ms_real();
                continue;
            }
            // Mutex waiters: wake deadline-expired ones (lock timeout).
            if (g_mutex_count > 0) {
                int mw = mutex_wait_poll_and_wake();
                if (mw > 0) {
                    last_now = now_ms_real();
                    continue;
                }
            }
            int64_t next_wake = timer_peek_next_wake();
            if (next_wake >= 0) {
#ifdef _WIN32
                Sleep((DWORD)(next_wake > last_now ? next_wake - last_now : 0));
#else
                usleep((useconds_t)(next_wake > last_now ? (next_wake - last_now) * 1000 : 0));
#endif
                last_now = now_ms_real();
                continue;
            }
            // True deadlock: report cycles if detected.
            if (datara_deadlock_detect()) {
                fprintf(stderr, "[DEADLOCK] scheduler halted: %lld of %lld fibers done\n",
                        (long long)g_done_count, (long long)g_fiber_count);
            }
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
    g_io_waiter_count = 0;
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

// ============================================================================
// Fiber-aware non-blocking socket recv (PLAN_V152 N1.1). Try immediately;
// on would-block park the calling fiber in the IO-wait list — the
// scheduler's select() slice (io_wait_poll_and_wake) wakes it when the fd
// is readable or the deadline expires. Returns an Outcome<Str> object in
// the datara_rt_outcome_build layout ([0]=is_success,[1]=value,[2]=error).
// ============================================================================

void* datara_rt_fiber_net_recv(int64_t sock, int64_t max_bytes, int64_t timeout_ms) {
    if (sock < 0) return datara_rt_outcome_build(0, "", "invalid socket handle");
    Fiber* self = g_running;
    char buf[8192];
    int cap = max_bytes > 0 && max_bytes < 8192 ? (int)max_bytes : 8192;
    for (;;) {
#ifdef _WIN32
        int n = recv((SOCKET)sock, buf, cap, 0);
        if (n == SOCKET_ERROR) {
            int err = WSAGetLastError();
            if (err != WSAEWOULDBLOCK && err != WSAEINPROGRESS) {
                return datara_rt_outcome_build(0, "", "socket recv error");
            }
        } else {
            buf[n <= cap ? n : cap] = 0;
            return datara_rt_outcome_build(n > 0 ? 1 : 0, buf, n > 0 ? "" : "closed");
        }
#else
        ssize_t n = recv((int)sock, buf, (size_t)cap, 0);
        if (n < 0) {
            int err = errno;
            if (err != EWOULDBLOCK && err != EAGAIN && err != EINPROGRESS) {
                return datara_rt_outcome_build(0, "", "socket recv error");
            }
        } else {
            buf[n] = 0;
            return datara_rt_outcome_build(n > 0 ? 1 : 0, buf, n > 0 ? "" : "closed");
        }
#endif
        // Would-block: park until readable / deadline.
        if (!self) {
            // Outside a fiber: fall back to a blocking recv with a short
            // timeout via select (main-thread compat).
#ifdef _WIN32
            fd_set rfds; FD_ZERO(&rfds); FD_SET((SOCKET)sock, &rfds);
            struct timeval tv = { (long)(timeout_ms / 1000), (long)((timeout_ms % 1000) * 1000) };
            select((int)sock + 1, &rfds, NULL, NULL, &tv);
#else
            fd_set rfds; FD_ZERO(&rfds); FD_SET((int)sock, &rfds);
            struct timeval tv = { timeout_ms / 1000, (timeout_ms % 1000) * 1000 };
            select((int)sock + 1, &rfds, NULL, NULL, &tv);
#endif
            continue;
        }
        self->state = FIB_WAITING_IO;
        self->wait_fd = sock;
        self->wait_deadline = timeout_ms > 0 ? now_ms_real() + timeout_ms : 0;
        io_wait_add(self);
#ifdef _WIN32
        SwitchToFiber(g_scheduler_fiber);
#else
        swapcontext((ucontext_t*)self->os_fiber, g_scheduler_ctx);
#endif
        // Woken: loop back and retry recv.
    }
}
