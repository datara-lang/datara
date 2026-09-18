//! Runtime systems, concurrency, and architecture primitives registration for Cranelift JIT.
//!
//! Provides symbol mappings for Cranelift JIT (`forgen run`) so that execution
//! parity with AOT (`forgen build`) is 100% maintained for threading (`spawn`, `join`),
//! channels (`channel_*`, `Channel_*`), slices (`slice_*`, `SliceView_*`),
//! endian conversion (`bswap*`, `hton*`, `ntoh*`), scratchpad memory, and hardware volatile primitives.

use cranelift_jit::JITBuilder;
use std::os::raw::c_char;

unsafe extern "C" {
    // Endian conversion intrinsics
    pub fn datara_sys_hton16(val: i64) -> i64;
    pub fn datara_sys_ntoh16(val: i64) -> i64;
    pub fn datara_sys_hton32(val: i64) -> i64;
    pub fn datara_sys_ntoh32(val: i64) -> i64;
    pub fn datara_sys_hton64(val: i64) -> i64;
    pub fn datara_sys_ntoh64(val: i64) -> i64;
    pub fn datara_sys_bswap16(val: i64) -> i64;
    pub fn datara_sys_bswap32(val: i64) -> i64;
    pub fn datara_sys_bswap64(val: i64) -> i64;

    // Zero-copy SliceView operations
    pub fn datara_sys_slice_from_buffer(ptr: *mut u8, len: i64) -> *mut u8;
    pub fn datara_sys_slice_alloc(len: i64) -> *mut u8;
    pub fn datara_sys_slice_free(slice_ptr: *mut u8);
    pub fn datara_sys_slice_len(slice_ptr: *mut u8) -> i64;
    pub fn datara_sys_slice_get_byte(slice_ptr: *mut u8, offset: i64) -> i64;
    pub fn datara_sys_slice_set_byte(slice_ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_sys_slice_read_u16_be(slice_ptr: *mut u8, offset: i64) -> i64;
    pub fn datara_sys_slice_read_u16_le(slice_ptr: *mut u8, offset: i64) -> i64;
    pub fn datara_sys_slice_read_u32_be(slice_ptr: *mut u8, offset: i64) -> i64;
    pub fn datara_sys_slice_read_u32_le(slice_ptr: *mut u8, offset: i64) -> i64;
    pub fn datara_sys_slice_read_u64_be(slice_ptr: *mut u8, offset: i64) -> i64;
    pub fn datara_sys_slice_read_u64_le(slice_ptr: *mut u8, offset: i64) -> i64;
    pub fn datara_sys_slice_write_u16_be(slice_ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_sys_slice_write_u16_le(slice_ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_sys_slice_write_u32_be(slice_ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_sys_slice_write_u32_le(slice_ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_sys_slice_write_u64_be(slice_ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_sys_slice_write_u64_le(slice_ptr: *mut u8, offset: i64, val: i64);
    pub fn datara_sys_slice_subslice(slice_ptr: *mut u8, offset: i64, len: i64) -> *mut u8;

    // Hardware MMIO & Volatile Access
    pub fn datara_hw_volatile_read8(ptr: *const u8) -> i64;
    pub fn datara_hw_volatile_read16(ptr: *const u8) -> i64;
    pub fn datara_hw_volatile_read32(ptr: *const u8) -> i64;
    pub fn datara_hw_volatile_read64(ptr: *const u8) -> i64;
    pub fn datara_hw_volatile_write8(ptr: *mut u8, val: i64);
    pub fn datara_hw_volatile_write16(ptr: *mut u8, val: i64);
    pub fn datara_hw_volatile_write32(ptr: *mut u8, val: i64);
    pub fn datara_hw_volatile_write64(ptr: *mut u8, val: i64);
    pub fn datara_hw_volatile_ptr(ptr: *mut u8) -> *mut u8;
    pub fn datara_hw_typed_zero_init(size: i64) -> *mut u8;

    // Hardware Memory Fences
    pub fn datara_hw_atomic_fence_acquire();
    pub fn datara_hw_atomic_fence_release();
    pub fn datara_hw_atomic_fence_acq_rel();
    pub fn datara_hw_atomic_fence_seq_cst();
    pub fn datara_hw_atomic_fence(order: *const c_char);

    // Concurrency & Thread Handles
    pub fn datara_thread_spawn(fn_ptr: *const (), arg: *mut ()) -> *mut ();
    pub fn spawn(fn_ptr: *const ()) -> *mut ();
    pub fn datara_thread_join(handle: *mut (), timeout_ms: i64, out_res: *mut i64) -> i32;
    pub fn ThreadHandle_join(handle: *mut ()) -> i64;
    pub fn join(handle: *mut ()) -> i64;
    pub fn join_timeout(handle: *mut (), timeout_ms: i64) -> i64;
    pub fn ThreadHandle_free(handle: *mut ());
    pub fn datara_thread_free(handle: *mut ());

    // Channels
    pub fn datara_channel_create(capacity: i64) -> *mut ();
    pub fn channel_create(capacity: i64) -> *mut ();
    pub fn channel_new() -> *mut ();
    pub fn datara_channel_send(ch: *mut (), val: i64) -> i32;
    pub fn Channel_send(ch: *mut (), val: i64) -> i32;
    pub fn channel_send(ch: *mut (), val: i64) -> i32;
    pub fn datara_channel_recv(ch: *mut (), timeout_ms: i64, out_val: *mut i64) -> i32;
    pub fn Channel_recv(ch: *mut ()) -> i64;
    pub fn channel_recv(ch: *mut ()) -> i64;
    pub fn datara_channel_try_recv(ch: *mut (), out_val: *mut i64) -> i32;
    pub fn Channel_try_recv(ch: *mut ()) -> i64;
    pub fn channel_try_recv(ch: *mut ()) -> i64;
    pub fn datara_channel_len(ch: *mut ()) -> i64;
    pub fn Channel_len(ch: *mut ()) -> i64;
    pub fn channel_len(ch: *mut ()) -> i64;
    pub fn datara_channel_close(ch: *mut ());
    pub fn Channel_close(ch: *mut ());
    pub fn channel_close(ch: *mut ());
    pub fn datara_channel_free(ch: *mut ());
    pub fn Channel_free(ch: *mut ());

    // Parallel For
    pub fn datara_parallel_for(start: i64, end: i64, body: *const (), ctx: *mut ());
    pub fn parallel_for(start: i64, end: i64, body: *const (), ctx: *mut ());

    // Scratchpad Memory
    pub fn datara_scratch_enter() -> i64;
    pub fn scratch_enter() -> i64;
    pub fn datara_scratch_alloc(size: i64, align: i64) -> *mut u8;
    pub fn scratch_alloc(size: i64, align: i64) -> *mut u8;
    pub fn datara_scratch_exit(watermark: i64);
    pub fn scratch_exit(watermark: i64);
    pub fn datara_scratch_promote(ptr: *mut u8, size: i64) -> *mut u8;
    pub fn scratch_promote(ptr: *mut u8, size: i64) -> *mut u8;
}

pub fn register_system_and_concurrency_symbols(builder: &mut JITBuilder) {
    macro_rules! reg {
        ($sym:expr, $func:ident) => {
            builder.symbol($sym, $func as *const u8);
        };
    }

    // Endianness
    reg!("hton16", datara_sys_hton16);
    reg!("datara_sys_hton16", datara_sys_hton16);
    reg!("ntoh16", datara_sys_ntoh16);
    reg!("datara_sys_ntoh16", datara_sys_ntoh16);
    reg!("hton32", datara_sys_hton32);
    reg!("datara_sys_hton32", datara_sys_hton32);
    reg!("ntoh32", datara_sys_ntoh32);
    reg!("datara_sys_ntoh32", datara_sys_ntoh32);
    reg!("hton64", datara_sys_hton64);
    reg!("datara_sys_hton64", datara_sys_hton64);
    reg!("ntoh64", datara_sys_ntoh64);
    reg!("datara_sys_ntoh64", datara_sys_ntoh64);
    reg!("bswap16", datara_sys_bswap16);
    reg!("datara_sys_bswap16", datara_sys_bswap16);
    reg!("bswap32", datara_sys_bswap32);
    reg!("datara_sys_bswap32", datara_sys_bswap32);
    reg!("bswap64", datara_sys_bswap64);
    reg!("datara_sys_bswap64", datara_sys_bswap64);

    // Slices & SliceView
    reg!("slice_alloc", datara_sys_slice_alloc);
    reg!("datara_sys_slice_alloc", datara_sys_slice_alloc);
    reg!("slice_from_buffer", datara_sys_slice_from_buffer);
    reg!("datara_sys_slice_from_buffer", datara_sys_slice_from_buffer);
    reg!("slice_free", datara_sys_slice_free);
    reg!("SliceView_free", datara_sys_slice_free);
    reg!("datara_sys_slice_free", datara_sys_slice_free);
    reg!("SliceView_len", datara_sys_slice_len);
    reg!("datara_sys_slice_len", datara_sys_slice_len);
    reg!("SliceView_get_byte", datara_sys_slice_get_byte);
    reg!("datara_sys_slice_get_byte", datara_sys_slice_get_byte);
    reg!("SliceView_set_byte", datara_sys_slice_set_byte);
    reg!("datara_sys_slice_set_byte", datara_sys_slice_set_byte);
    reg!("SliceView_read_u16_be", datara_sys_slice_read_u16_be);
    reg!("datara_sys_slice_read_u16_be", datara_sys_slice_read_u16_be);
    reg!("SliceView_read_u16_le", datara_sys_slice_read_u16_le);
    reg!("datara_sys_slice_read_u16_le", datara_sys_slice_read_u16_le);
    reg!("SliceView_read_u32_be", datara_sys_slice_read_u32_be);
    reg!("datara_sys_slice_read_u32_be", datara_sys_slice_read_u32_be);
    reg!("SliceView_read_u32_le", datara_sys_slice_read_u32_le);
    reg!("datara_sys_slice_read_u32_le", datara_sys_slice_read_u32_le);
    reg!("SliceView_read_u64_be", datara_sys_slice_read_u64_be);
    reg!("datara_sys_slice_read_u64_be", datara_sys_slice_read_u64_be);
    reg!("SliceView_read_u64_le", datara_sys_slice_read_u64_le);
    reg!("datara_sys_slice_read_u64_le", datara_sys_slice_read_u64_le);
    reg!("SliceView_write_u16_be", datara_sys_slice_write_u16_be);
    reg!(
        "datara_sys_slice_write_u16_be",
        datara_sys_slice_write_u16_be
    );
    reg!("SliceView_write_u16_le", datara_sys_slice_write_u16_le);
    reg!(
        "datara_sys_slice_write_u16_le",
        datara_sys_slice_write_u16_le
    );
    reg!("SliceView_write_u32_be", datara_sys_slice_write_u32_be);
    reg!(
        "datara_sys_slice_write_u32_be",
        datara_sys_slice_write_u32_be
    );
    reg!("SliceView_write_u32_le", datara_sys_slice_write_u32_le);
    reg!(
        "datara_sys_slice_write_u32_le",
        datara_sys_slice_write_u32_le
    );
    reg!("SliceView_write_u64_be", datara_sys_slice_write_u64_be);
    reg!(
        "datara_sys_slice_write_u64_be",
        datara_sys_slice_write_u64_be
    );
    reg!("SliceView_write_u64_le", datara_sys_slice_write_u64_le);
    reg!(
        "datara_sys_slice_write_u64_le",
        datara_sys_slice_write_u64_le
    );
    reg!("SliceView_subslice", datara_sys_slice_subslice);
    reg!("datara_sys_slice_subslice", datara_sys_slice_subslice);

    // Volatile MMIO
    reg!("volatile_read8", datara_hw_volatile_read8);
    reg!("datara_hw_volatile_read8", datara_hw_volatile_read8);
    reg!("VolatilePtr_read8", datara_hw_volatile_read8);
    reg!("volatile_read16", datara_hw_volatile_read16);
    reg!("datara_hw_volatile_read16", datara_hw_volatile_read16);
    reg!("VolatilePtr_read16", datara_hw_volatile_read16);
    reg!("volatile_read32", datara_hw_volatile_read32);
    reg!("datara_hw_volatile_read32", datara_hw_volatile_read32);
    reg!("VolatilePtr_read32", datara_hw_volatile_read32);
    reg!("volatile_read64", datara_hw_volatile_read64);
    reg!("datara_hw_volatile_read64", datara_hw_volatile_read64);
    reg!("VolatilePtr_read64", datara_hw_volatile_read64);
    reg!("volatile_write8", datara_hw_volatile_write8);
    reg!("datara_hw_volatile_write8", datara_hw_volatile_write8);
    reg!("VolatilePtr_write8", datara_hw_volatile_write8);
    reg!("volatile_write16", datara_hw_volatile_write16);
    reg!("datara_hw_volatile_write16", datara_hw_volatile_write16);
    reg!("VolatilePtr_write16", datara_hw_volatile_write16);
    reg!("volatile_write32", datara_hw_volatile_write32);
    reg!("datara_hw_volatile_write32", datara_hw_volatile_write32);
    reg!("VolatilePtr_write32", datara_hw_volatile_write32);
    reg!("volatile_write64", datara_hw_volatile_write64);
    reg!("datara_hw_volatile_write64", datara_hw_volatile_write64);
    reg!("VolatilePtr_write64", datara_hw_volatile_write64);
    reg!("volatile_ptr", datara_hw_volatile_ptr);
    reg!("datara_hw_volatile_ptr", datara_hw_volatile_ptr);
    reg!("typed_zero_init", datara_hw_typed_zero_init);
    reg!("datara_hw_typed_zero_init", datara_hw_typed_zero_init);

    // Hardware Fences
    reg!("atomic_fence_acquire", datara_hw_atomic_fence_acquire);
    reg!(
        "datara_hw_atomic_fence_acquire",
        datara_hw_atomic_fence_acquire
    );
    reg!("atomic_fence_release", datara_hw_atomic_fence_release);
    reg!(
        "datara_hw_atomic_fence_release",
        datara_hw_atomic_fence_release
    );
    reg!("atomic_fence_acq_rel", datara_hw_atomic_fence_acq_rel);
    reg!(
        "datara_hw_atomic_fence_acq_rel",
        datara_hw_atomic_fence_acq_rel
    );
    reg!("atomic_fence_seq_cst", datara_hw_atomic_fence_seq_cst);
    reg!(
        "datara_hw_atomic_fence_seq_cst",
        datara_hw_atomic_fence_seq_cst
    );
    reg!("atomic_fence", datara_hw_atomic_fence);
    reg!("datara_hw_atomic_fence", datara_hw_atomic_fence);

    // Concurrency / Threading
    reg!("spawn", spawn);
    reg!("datara_thread_spawn_wrap", spawn);
    reg!("join", join);
    reg!("ThreadHandle_join", ThreadHandle_join);
    reg!("join_timeout", join_timeout);
    reg!("ThreadHandle_join_timeout", join_timeout);
    reg!("ThreadHandle_free", ThreadHandle_free);
    reg!("datara_thread_free", datara_thread_free);

    // Channels
    reg!("channel_create", channel_create);
    reg!("datara_channel_create", datara_channel_create);
    reg!("channel_new", channel_new);
    reg!("Channel_new", channel_new);
    reg!("channel_send", channel_send);
    reg!("Channel_send", Channel_send);
    reg!("datara_channel_send_wrap", Channel_send);
    reg!("channel_recv", channel_recv);
    reg!("Channel_recv", Channel_recv);
    reg!("channel_try_recv", channel_try_recv);
    reg!("Channel_try_recv", Channel_try_recv);
    reg!("channel_close", channel_close);
    reg!("Channel_close", Channel_close);
    reg!("channel_len", channel_len);
    reg!("Channel_len", Channel_len);
    reg!("Channel_free", Channel_free);
    reg!("datara_channel_free", datara_channel_free);

    // Parallel For
    reg!("parallel_for", parallel_for);
    reg!("datara_parallel_for", datara_parallel_for);

    // Scratchpad Arena
    reg!("scratch_enter", scratch_enter);
    reg!("datara_scratch_enter", datara_scratch_enter);
    reg!("scratch_alloc", scratch_alloc);
    reg!("datara_scratch_alloc", datara_scratch_alloc);
    reg!("scratch_exit", scratch_exit);
    reg!("datara_scratch_exit", datara_scratch_exit);
    reg!("scratch_promote", scratch_promote);
    reg!("datara_scratch_promote", datara_scratch_promote);
}
