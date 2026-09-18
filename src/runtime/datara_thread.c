#include "datara_thread.h"
#include "datara_runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
  #ifndef WIN32_LEAN_AND_MEAN
    #define WIN32_LEAN_AND_MEAN
  #endif
  #include <windows.h>
#else
  #include <pthread.h>
  #include <unistd.h>
  #include <sys/time.h>
  #include <time.h>
  #include <errno.h>
#endif

// ============================================================================
// 1. Thread Handle Implementation
// ============================================================================

struct DataraThreadHandle {
#ifdef _WIN32
    HANDLE handle;
    DWORD thread_id;
#else
    pthread_t thread;
#endif
    void* (*entry_fn)(void*);
    void* arg;
    int64_t result;
    int32_t finished;
    int32_t joined;
    int32_t detached;
#ifdef _WIN32
    CRITICAL_SECTION cs;
    CONDITION_VARIABLE cv;
#else
    pthread_mutex_t mutex;
    pthread_cond_t cond;
#endif
};

#ifdef _WIN32
static DWORD WINAPI datara_thread_proxy(LPVOID param) {
    DataraThreadHandle* th = (DataraThreadHandle*)param;
    void* ret = th->entry_fn(th->arg);
    EnterCriticalSection(&th->cs);
    th->result = (int64_t)(intptr_t)ret;
    th->finished = 1;
    WakeAllConditionVariable(&th->cv);
    LeaveCriticalSection(&th->cs);
    return 0;
}
#else
static void* datara_thread_proxy(void* param) {
    DataraThreadHandle* th = (DataraThreadHandle*)param;
    void* ret = th->entry_fn(th->arg);
    pthread_mutex_lock(&th->mutex);
    th->result = (int64_t)(intptr_t)ret;
    th->finished = 1;
    pthread_cond_broadcast(&th->cond);
    pthread_mutex_unlock(&th->mutex);
    return NULL;
}
#endif

DataraThreadHandle* datara_thread_spawn(void* (*fn)(void*), void* arg) {
    if (!fn) return NULL;
    DataraThreadHandle* th = (DataraThreadHandle*)calloc(1, sizeof(DataraThreadHandle));
    if (!th) return NULL;

    th->entry_fn = fn;
    th->arg = arg;
    th->finished = 0;
    th->joined = 0;
    th->detached = 0;

#ifdef _WIN32
    InitializeCriticalSection(&th->cs);
    InitializeConditionVariable(&th->cv);
    th->handle = CreateThread(NULL, 0, datara_thread_proxy, th, 0, &th->thread_id);
    if (!th->handle) {
        DeleteCriticalSection(&th->cs);
        free(th);
        return NULL;
    }
#else
    pthread_mutex_init(&th->mutex, NULL);
    pthread_cond_init(&th->cond, NULL);
    if (pthread_create(&th->thread, NULL, datara_thread_proxy, th) != 0) {
        pthread_mutex_destroy(&th->mutex);
        pthread_cond_destroy(&th->cond);
        free(th);
        return NULL;
    }
#endif

    return th;
}

int32_t datara_thread_join(DataraThreadHandle* handle, int64_t timeout_ms, int64_t* out_res) {
    if (!handle) return -1;

#ifdef _WIN32
    EnterCriticalSection(&handle->cs);
    if (!handle->finished) {
        if (timeout_ms < 0) {
            while (!handle->finished) {
                SleepConditionVariableCS(&handle->cv, &handle->cs, INFINITE);
            }
        } else if (timeout_ms == 0) {
            if (!handle->finished) {
                LeaveCriticalSection(&handle->cs);
                return 1; // timeout
            }
        } else {
            DWORD remaining = (DWORD)timeout_ms;
            while (!handle->finished && remaining > 0) {
                DWORD t0 = GetTickCount();
                if (!SleepConditionVariableCS(&handle->cv, &handle->cs, remaining)) {
                    break;
                }
                DWORD elapsed = GetTickCount() - t0;
                if (elapsed >= remaining) remaining = 0;
                else remaining -= elapsed;
            }
            if (!handle->finished) {
                LeaveCriticalSection(&handle->cs);
                return 1; // timeout
            }
        }
    }
    if (out_res) *out_res = handle->result;
    handle->joined = 1;
    LeaveCriticalSection(&handle->cs);
    return 0;
#else
    pthread_mutex_lock(&handle->mutex);
    if (!handle->finished) {
        if (timeout_ms < 0) {
            while (!handle->finished) {
                pthread_cond_wait(&handle->cond, &handle->mutex);
            }
        } else if (timeout_ms == 0) {
            if (!handle->finished) {
                pthread_mutex_unlock(&handle->mutex);
                return 1;
            }
        } else {
            struct timeval now;
            gettimeofday(&now, NULL);
            struct timespec ts;
            ts.tv_sec = now.tv_sec + timeout_ms / 1000;
            ts.tv_nsec = (now.tv_usec + (timeout_ms % 1000) * 1000) * 1000;
            if (ts.tv_nsec >= 1000000000L) {
                ts.tv_sec += 1;
                ts.tv_nsec -= 1000000000L;
            }
            while (!handle->finished) {
                int rc = pthread_cond_timedwait(&handle->cond, &handle->mutex, &ts);
                if (rc == ETIMEDOUT) break;
            }
            if (!handle->finished) {
                pthread_mutex_unlock(&handle->mutex);
                return 1;
            }
        }
    }
    if (out_res) *out_res = handle->result;
    handle->joined = 1;
    pthread_mutex_unlock(&handle->mutex);
    return 0;
#endif
}

int32_t datara_thread_is_alive(DataraThreadHandle* handle) {
    if (!handle) return 0;
#ifdef _WIN32
    EnterCriticalSection(&handle->cs);
    int32_t alive = !handle->finished;
    LeaveCriticalSection(&handle->cs);
    return alive;
#else
    pthread_mutex_lock(&handle->mutex);
    int32_t alive = !handle->finished;
    pthread_mutex_unlock(&handle->mutex);
    return alive;
#endif
}

void datara_thread_free(DataraThreadHandle* handle) {
    if (!handle) return;
#ifdef _WIN32
    if (!handle->joined) {
        CloseHandle(handle->handle);
    }
    DeleteCriticalSection(&handle->cs);
#else
    if (!handle->joined) {
        pthread_detach(handle->thread);
    }
    pthread_mutex_destroy(&handle->mutex);
    pthread_cond_destroy(&handle->cond);
#endif
    free(handle);
}


// ============================================================================
// 2. Channel Primitives (Bounded MPSC Queue)
// ============================================================================

struct DataraChannel {
    int64_t* ring;
    int64_t capacity;
    int64_t head;
    int64_t tail;
    int64_t count;
    int32_t closed;
#ifdef _WIN32
    CRITICAL_SECTION cs;
    CONDITION_VARIABLE cv_not_empty;
    CONDITION_VARIABLE cv_not_full;
#else
    pthread_mutex_t mutex;
    pthread_cond_t cond_not_empty;
    pthread_cond_t cond_not_full;
#endif
};

DataraChannel* datara_channel_create(int64_t capacity) {
    if (capacity <= 0) capacity = 256;
    DataraChannel* ch = (DataraChannel*)calloc(1, sizeof(DataraChannel));
    if (!ch) return NULL;

    ch->ring = (int64_t*)malloc(sizeof(int64_t) * (size_t)capacity);
    if (!ch->ring) {
        free(ch);
        return NULL;
    }

    ch->capacity = capacity;
    ch->head = 0;
    ch->tail = 0;
    ch->count = 0;
    ch->closed = 0;

#ifdef _WIN32
    InitializeCriticalSection(&ch->cs);
    InitializeConditionVariable(&ch->cv_not_empty);
    InitializeConditionVariable(&ch->cv_not_full);
#else
    pthread_mutex_init(&ch->mutex, NULL);
    pthread_cond_init(&ch->cond_not_empty, NULL);
    pthread_cond_init(&ch->cond_not_full, NULL);
#endif

    return ch;
}

int32_t datara_channel_send(DataraChannel* ch, int64_t val) {
    if (!ch) return 1;

#ifdef _WIN32
    EnterCriticalSection(&ch->cs);
    while (ch->count >= ch->capacity && !ch->closed) {
        SleepConditionVariableCS(&ch->cv_not_full, &ch->cs, INFINITE);
    }
    if (ch->closed) {
        LeaveCriticalSection(&ch->cs);
        return 1;
    }
    ch->ring[ch->tail] = val;
    ch->tail = (ch->tail + 1) % ch->capacity;
    ch->count++;
    WakeConditionVariable(&ch->cv_not_empty);
    LeaveCriticalSection(&ch->cs);
    return 0;
#else
    pthread_mutex_lock(&ch->mutex);
    while (ch->count >= ch->capacity && !ch->closed) {
        pthread_cond_wait(&ch->cond_not_full, &ch->mutex);
    }
    if (ch->closed) {
        pthread_mutex_unlock(&ch->mutex);
        return 1;
    }
    ch->ring[ch->tail] = val;
    ch->tail = (ch->tail + 1) % ch->capacity;
    ch->count++;
    pthread_cond_signal(&ch->cond_not_empty);
    pthread_mutex_unlock(&ch->mutex);
    return 0;
#endif
}

int32_t datara_channel_recv(DataraChannel* ch, int64_t timeout_ms, int64_t* out_val) {
    if (!ch) return 2;

#ifdef _WIN32
    EnterCriticalSection(&ch->cs);
    if (timeout_ms < 0) {
        while (ch->count == 0 && !ch->closed) {
            SleepConditionVariableCS(&ch->cv_not_empty, &ch->cs, INFINITE);
        }
    } else if (timeout_ms == 0) {
        // Non-blocking
    } else {
        DWORD remaining = (DWORD)timeout_ms;
        while (ch->count == 0 && !ch->closed && remaining > 0) {
            DWORD t0 = GetTickCount();
            if (!SleepConditionVariableCS(&ch->cv_not_empty, &ch->cs, remaining)) break;
            DWORD elapsed = GetTickCount() - t0;
            if (elapsed >= remaining) remaining = 0;
            else remaining -= elapsed;
        }
    }

    if (ch->count > 0) {
        int64_t val = ch->ring[ch->head];
        ch->head = (ch->head + 1) % ch->capacity;
        ch->count--;
        if (out_val) *out_val = val;
        WakeConditionVariable(&ch->cv_not_full);
        LeaveCriticalSection(&ch->cs);
        return 0;
    }

    int32_t is_closed = ch->closed;
    LeaveCriticalSection(&ch->cs);
    return is_closed ? 2 : 1;
#else
    pthread_mutex_lock(&ch->mutex);
    if (timeout_ms < 0) {
        while (ch->count == 0 && !ch->closed) {
            pthread_cond_wait(&ch->cond_not_empty, &ch->mutex);
        }
    } else if (timeout_ms == 0) {
        // Non-blocking
    } else {
        struct timeval now;
        gettimeofday(&now, NULL);
        struct timespec ts;
        ts.tv_sec = now.tv_sec + timeout_ms / 1000;
        ts.tv_nsec = (now.tv_usec + (timeout_ms % 1000) * 1000) * 1000;
        if (ts.tv_nsec >= 1000000000L) {
            ts.tv_sec += 1;
            ts.tv_nsec -= 1000000000L;
        }
        while (ch->count == 0 && !ch->closed) {
            if (pthread_cond_timedwait(&ch->cond_not_empty, &ch->mutex, &ts) == ETIMEDOUT) break;
        }
    }

    if (ch->count > 0) {
        int64_t val = ch->ring[ch->head];
        ch->head = (ch->head + 1) % ch->capacity;
        ch->count--;
        if (out_val) *out_val = val;
        pthread_cond_signal(&ch->cond_not_full);
        pthread_mutex_unlock(&ch->mutex);
        return 0;
    }

    int32_t is_closed = ch->closed;
    pthread_mutex_unlock(&ch->mutex);
    return is_closed ? 2 : 1;
#endif
}

int32_t datara_channel_try_recv(DataraChannel* ch, int64_t* out_val) {
    return datara_channel_recv(ch, 0, out_val);
}

int64_t datara_channel_len(DataraChannel* ch) {
    if (!ch) return 0;
#ifdef _WIN32
    EnterCriticalSection(&ch->cs);
    int64_t cnt = ch->count;
    LeaveCriticalSection(&ch->cs);
    return cnt;
#else
    pthread_mutex_lock(&ch->mutex);
    int64_t cnt = ch->count;
    pthread_mutex_unlock(&ch->mutex);
    return cnt;
#endif
}

void datara_channel_close(DataraChannel* ch) {
    if (!ch) return;
#ifdef _WIN32
    EnterCriticalSection(&ch->cs);
    ch->closed = 1;
    WakeAllConditionVariable(&ch->cv_not_empty);
    WakeAllConditionVariable(&ch->cv_not_full);
    LeaveCriticalSection(&ch->cs);
#else
    pthread_mutex_lock(&ch->mutex);
    ch->closed = 1;
    pthread_cond_broadcast(&ch->cond_not_empty);
    pthread_cond_broadcast(&ch->cond_not_full);
    pthread_mutex_unlock(&ch->mutex);
#endif
}

int32_t datara_channel_is_closed(DataraChannel* ch) {
    if (!ch) return 1;
#ifdef _WIN32
    EnterCriticalSection(&ch->cs);
    int32_t c = ch->closed;
    LeaveCriticalSection(&ch->cs);
    return c;
#else
    pthread_mutex_lock(&ch->mutex);
    int32_t c = ch->closed;
    pthread_mutex_unlock(&ch->mutex);
    return c;
#endif
}

void datara_channel_free(DataraChannel* ch) {
    if (!ch) return;
    datara_channel_close(ch);
#ifdef _WIN32
    DeleteCriticalSection(&ch->cs);
#else
    pthread_mutex_destroy(&ch->mutex);
    pthread_cond_destroy(&ch->cond_not_empty);
    pthread_cond_destroy(&ch->cond_not_full);
#endif
    if (ch->ring) free(ch->ring);
    free(ch);
}


// ============================================================================
// 3. Parallel Range / Data Parallelism (parallel_for)
// ============================================================================

typedef struct {
    int64_t start;
    int64_t end;
    DataraParallelBodyFn body;
    void* ctx;
} ParallelChunkArg;

static void* parallel_chunk_worker(void* raw_arg) {
    ParallelChunkArg* arg = (ParallelChunkArg*)raw_arg;
    for (int64_t i = arg->start; i < arg->end; ++i) {
        arg->body(i, arg->ctx);
    }
    return NULL;
}

void datara_parallel_for(int64_t start, int64_t end, DataraParallelBodyFn body, void* ctx) {
    if (start >= end || !body) return;

    int64_t total = end - start;
    if (total <= 128) {
        // Sequential fallback for small loop sizes to avoid thread dispatch overhead
        for (int64_t i = start; i < end; ++i) {
            body(i, ctx);
        }
        return;
    }

#ifdef _WIN32
    SYSTEM_INFO sysinfo;
    GetSystemInfo(&sysinfo);
    int num_cores = (int)sysinfo.dwNumberOfProcessors;
#else
    int num_cores = (int)sysconf(_SC_NPROCESSORS_ONLN);
#endif
    if (num_cores < 1) num_cores = 1;
    if (num_cores > 32) num_cores = 32;

    int64_t chunk_size = (total + num_cores - 1) / num_cores;
    DataraThreadHandle* threads[32];
    ParallelChunkArg args[32];
    int spawned = 0;

    int64_t cur = start;
    for (int i = 0; i < num_cores && cur < end; ++i) {
        int64_t next = cur + chunk_size;
        if (next > end) next = end;

        args[i].start = cur;
        args[i].end = next;
        args[i].body = body;
        args[i].ctx = ctx;

        if (i == 0) {
            // Main thread participates in the first chunk
            cur = next;
            continue;
        }

        threads[spawned] = datara_thread_spawn(parallel_chunk_worker, &args[i]);
        if (threads[spawned]) {
            spawned++;
            cur = next;
        } else {
            // Fallback for remaining items
            break;
        }
    }

    // Run first chunk on calling thread
    parallel_chunk_worker(&args[0]);

    // Join spawned chunks
    for (int i = 0; i < spawned; ++i) {
        datara_thread_join(threads[i], -1, NULL);
        datara_thread_free(threads[i]);
    }
}


// ============================================================================
// 4. Autonomous Scratchpad Memory (The Novel Datara Transient Lifecycles)
// ============================================================================

#define DATARA_SCRATCH_BYTES (4 * 1024 * 1024) // 4 MB per thread ring

typedef struct {
    uint8_t* buffer;
    size_t watermark;
    size_t capacity;
    int initialized;
} DataraScratchState;

#ifdef _WIN32
static __declspec(thread) DataraScratchState tl_scratch = { NULL, 0, 0, 0 };
#else
static __thread DataraScratchState tl_scratch = { NULL, 0, 0, 0 };
#endif

static void ensure_scratch_init(void) {
    if (!tl_scratch.initialized) {
        tl_scratch.buffer = (uint8_t*)malloc(DATARA_SCRATCH_BYTES);
        tl_scratch.capacity = tl_scratch.buffer ? DATARA_SCRATCH_BYTES : 0;
        tl_scratch.watermark = 0;
        tl_scratch.initialized = 1;
    }
}

int64_t datara_scratch_enter(void) {
    ensure_scratch_init();
    return (int64_t)tl_scratch.watermark;
}

void* datara_scratch_alloc(int64_t size, int64_t align) {
    ensure_scratch_init();
    if (size <= 0) return NULL;
    if (align < 8) align = 8;

    size_t current = tl_scratch.watermark;
    size_t aligned = (current + (size_t)(align - 1)) & ~((size_t)(align - 1));

    if (aligned + (size_t)size <= tl_scratch.capacity) {
        tl_scratch.watermark = aligned + (size_t)size;
        return (void*)(tl_scratch.buffer + aligned);
    }

    // Overflow fallback: safe dynamic heap allocation
    return malloc((size_t)size);
}

void datara_scratch_exit(int64_t watermark) {
    if (watermark >= 0 && (size_t)watermark <= tl_scratch.capacity) {
        tl_scratch.watermark = (size_t)watermark; // 0-ns instant rewind!
    }
}

void* datara_scratch_promote(void* ptr, int64_t size) {
    if (!ptr || size <= 0) return NULL;
    // If ptr is in scratchpad, deep clone to heap
    if (tl_scratch.buffer &&
        (uint8_t*)ptr >= tl_scratch.buffer &&
        (uint8_t*)ptr < tl_scratch.buffer + tl_scratch.capacity) {
        void* heap_ptr = malloc((size_t)size);
        if (heap_ptr) memcpy(heap_ptr, ptr, (size_t)size);
        return heap_ptr;
    }
    return ptr;
}

// ============================================================================
// 5. Direct Datara Call & Method Dispatch Wrappers
// ============================================================================

int32_t Channel_send(DataraChannel* ch, int64_t val) {
    return datara_channel_send(ch, val);
}

int64_t Channel_recv(DataraChannel* ch) {
    int64_t out_val = 0;
    int32_t rc = datara_channel_recv(ch, -1, &out_val);
    return (rc == 0) ? out_val : 0;
}

int64_t Channel_try_recv(DataraChannel* ch) {
    int64_t out_val = 0;
    int32_t rc = datara_channel_try_recv(ch, &out_val);
    return (rc == 0) ? out_val : 0;
}

void Channel_close(DataraChannel* ch) {
    datara_channel_close(ch);
}

int64_t Channel_len(DataraChannel* ch) {
    return datara_channel_len(ch);
}

void Channel_free(DataraChannel* ch) {
    datara_channel_free(ch);
}

int64_t ThreadHandle_join(DataraThreadHandle* th) {
    int64_t out_res = 0;
    int32_t rc = datara_thread_join(th, -1, &out_res);
    return (rc == 0) ? out_res : -1;
}

int64_t ThreadHandle_join_timeout(DataraThreadHandle* th, int64_t timeout_ms) {
    int64_t out_res = 0;
    int32_t rc = datara_thread_join(th, timeout_ms, &out_res);
    return (rc == 0) ? out_res : -1;
}

void ThreadHandle_free(DataraThreadHandle* th) {
    datara_thread_free(th);
}

DataraThreadHandle* spawn(void* (*fn)(void*)) {
    return datara_thread_spawn(fn, NULL);
}

int64_t join(DataraThreadHandle* th) {
    int64_t out_res = 0;
    int32_t rc = datara_thread_join(th, -1, &out_res);
    return (rc == 0) ? out_res : -1;
}

int64_t join_timeout(DataraThreadHandle* th, int64_t timeout_ms) {
    int64_t out_res = 0;
    int32_t rc = datara_thread_join(th, timeout_ms, &out_res);
    return (rc == 0) ? out_res : -1;
}

DataraChannel* channel_create(int64_t capacity) {
    return datara_channel_create(capacity);
}

DataraChannel* channel_new(void) {
    return datara_channel_create(256);
}

int32_t channel_send(DataraChannel* ch, int64_t val) {
    return datara_channel_send(ch, val);
}

int64_t channel_recv(DataraChannel* ch) {
    int64_t out_val = 0;
    int32_t rc = datara_channel_recv(ch, -1, &out_val);
    return (rc == 0) ? out_val : 0;
}

int64_t channel_try_recv(DataraChannel* ch) {
    int64_t out_val = 0;
    int32_t rc = datara_channel_try_recv(ch, &out_val);
    return (rc == 0) ? out_val : 0;
}

void channel_close(DataraChannel* ch) {
    datara_channel_close(ch);
}

int64_t channel_len(DataraChannel* ch) {
    return datara_channel_len(ch);
}

int64_t scratch_enter(void) {
    return datara_scratch_enter();
}

void* scratch_alloc(int64_t size, int64_t align) {
    return datara_scratch_alloc(size, align);
}

void scratch_exit(int64_t watermark) {
    datara_scratch_exit(watermark);
}

void* scratch_promote(void* ptr, int64_t size) {
    return datara_scratch_promote(ptr, size);
}

void parallel_for(int64_t start, int64_t end, DataraParallelBodyFn body, void* ctx) {
    datara_parallel_for(start, end, body, ctx);
}
