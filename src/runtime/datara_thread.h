#ifndef DATARA_THREAD_H
#define DATARA_THREAD_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// --- 1. Thread Handle Primitives ---

typedef struct DataraThreadHandle DataraThreadHandle;

// Spawn a new native OS thread executing `fn(arg)`.
// Returns a thread handle.
DataraThreadHandle* datara_thread_spawn(void* (*fn)(void*), void* arg);

// Wait for the thread to finish up to `timeout_ms` (-1 for infinite).
// Stores the return value in `*out_res` if not NULL.
// Returns: 0 on success, 1 on timeout, -1 on error/panic.
int32_t datara_thread_join(DataraThreadHandle* handle, int64_t timeout_ms, int64_t* out_res);

// Check whether the thread is still running (1 = running, 0 = terminated).
int32_t datara_thread_is_alive(DataraThreadHandle* handle);

// Free thread handle resources (auto-detaches if still running).
void datara_thread_free(DataraThreadHandle* handle);


// --- 2. Channel Primitives (Bounded MPSC Queue) ---

typedef struct DataraChannel DataraChannel;

// Create a new bounded channel with the given capacity (default 256 if <= 0).
DataraChannel* datara_channel_create(int64_t capacity);

// Send an int64 / pointer value into the channel. Blocks if channel is full.
// Returns: 0 on success, 1 if channel is closed.
int32_t datara_channel_send(DataraChannel* ch, int64_t val);

// Receive a value from the channel.
// timeout_ms: -1 = block forever, 0 = non-blocking, >0 = bounded wait.
// Returns: 0 on success (stores in *out_val), 1 on timeout/empty, 2 if closed and empty.
int32_t datara_channel_recv(DataraChannel* ch, int64_t timeout_ms, int64_t* out_val);

// Non-blocking receive helper.
// Returns: 0 on success, 1 if empty, 2 if closed.
int32_t datara_channel_try_recv(DataraChannel* ch, int64_t* out_val);

// Return number of pending items currently in the channel.
int64_t datara_channel_len(DataraChannel* ch);

// Close the channel for sending; wakes all waiting receivers.
void datara_channel_close(DataraChannel* ch);

// Check if channel is closed.
int32_t datara_channel_is_closed(DataraChannel* ch);

// Free the channel and all its resources.
void datara_channel_free(DataraChannel* ch);


// --- 3. Parallel Range / Data Parallelism ---

typedef void (*DataraParallelBodyFn)(int64_t index, void* ctx);

// Execute body(i, ctx) for i in [start, end) partitioned across available CPU cores.
void datara_parallel_for(int64_t start, int64_t end, DataraParallelBodyFn body, void* ctx);


// --- 4. Autonomous Scratchpad Memory (Zero-Allocation Scopes) ---

// Enter a scratchpad scope: records the current watermark.
int64_t datara_scratch_enter(void);

// Allocate `size` bytes with `align` alignment from thread-local scratch bump arena.
// 0-ns cost, never freed manually.
void* datara_scratch_alloc(int64_t size, int64_t align);

// Exit scratchpad scope: instantly rewinds bump pointer to watermark.
// Zero instruction overhead, zero memory leaks, zero fragmentation.
void datara_scratch_exit(int64_t watermark);

// Promote an object from scratchpad to long-lived heap if escaping the scope.
void* datara_scratch_promote(void* ptr, int64_t size);

// --- 5. Direct Datara Call & Method Dispatch Declarations ---
int32_t Channel_send(DataraChannel* ch, int64_t val);
int64_t Channel_recv(DataraChannel* ch);
int64_t Channel_try_recv(DataraChannel* ch);
void Channel_close(DataraChannel* ch);
int64_t Channel_len(DataraChannel* ch);
void Channel_free(DataraChannel* ch);
int64_t ThreadHandle_join(DataraThreadHandle* th);
void ThreadHandle_free(DataraThreadHandle* th);

DataraThreadHandle* spawn(void* (*fn)(void*));
int64_t join(DataraThreadHandle* th);
int64_t join_timeout(DataraThreadHandle* th, int64_t timeout_ms);
DataraChannel* channel_create(int64_t capacity);
DataraChannel* channel_new(void);
int32_t channel_send(DataraChannel* ch, int64_t val);
int64_t channel_recv(DataraChannel* ch);
int64_t channel_try_recv(DataraChannel* ch);
void channel_close(DataraChannel* ch);
int64_t channel_len(DataraChannel* ch);
int64_t scratch_enter(void);
void* scratch_alloc(int64_t size, int64_t align);
void scratch_exit(int64_t watermark);
void* scratch_promote(void* ptr, int64_t size);
void parallel_for(int64_t start, int64_t end, DataraParallelBodyFn body, void* ctx);

#ifdef __cplusplus
}
#endif

#endif // DATARA_THREAD_H
