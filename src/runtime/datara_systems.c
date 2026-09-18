#include "datara_systems.h"
#include <stdlib.h>
#include <string.h>

#if defined(_MSC_VER)
  #include <stdlib.h>
  #define BSWAP16(x) _byteswap_ushort((uint16_t)(x))
  #define BSWAP32(x) _byteswap_ulong((uint32_t)(x))
  #define BSWAP64(x) _byteswap_uint64((uint64_t)(x))
#else
  #define BSWAP16(x) __builtin_bswap16((uint16_t)(x))
  #define BSWAP32(x) __builtin_bswap32((uint32_t)(x))
  #define BSWAP64(x) __builtin_bswap64((uint64_t)(x))
#endif

// Endian byte-swap intrinsics
DATARA_SYS_API int64_t datara_sys_bswap16(int64_t val) {
    return (int64_t)BSWAP16((uint16_t)val);
}

DATARA_SYS_API int64_t datara_sys_bswap32(int64_t val) {
    return (int64_t)BSWAP32((uint32_t)val);
}

DATARA_SYS_API int64_t datara_sys_bswap64(int64_t val) {
    return (int64_t)BSWAP64((uint64_t)val);
}

static inline bool is_little_endian(void) {
    const uint16_t test = 0x0001;
    return *((const uint8_t*)&test) == 1;
}

DATARA_SYS_API int64_t datara_sys_hton16(int64_t val) {
    return is_little_endian() ? (int64_t)BSWAP16((uint16_t)val) : val;
}

DATARA_SYS_API int64_t datara_sys_ntoh16(int64_t val) {
    return is_little_endian() ? (int64_t)BSWAP16((uint16_t)val) : val;
}

DATARA_SYS_API int64_t datara_sys_hton32(int64_t val) {
    return is_little_endian() ? (int64_t)BSWAP32((uint32_t)val) : val;
}

DATARA_SYS_API int64_t datara_sys_ntoh32(int64_t val) {
    return is_little_endian() ? (int64_t)BSWAP32((uint32_t)val) : val;
}

DATARA_SYS_API int64_t datara_sys_hton64(int64_t val) {
    return is_little_endian() ? (int64_t)BSWAP64((uint64_t)val) : val;
}

DATARA_SYS_API int64_t datara_sys_ntoh64(int64_t val) {
    return is_little_endian() ? (int64_t)BSWAP64((uint64_t)val) : val;
}

// SliceView creation and operations
DATARA_SYS_API void* datara_sys_slice_from_buffer(void* ptr, int64_t len) {
    DataraSliceView* slice = (DataraSliceView*)malloc(sizeof(DataraSliceView));
    if (!slice) return NULL;
    slice->ptr = (uint8_t*)ptr;
    slice->len = len < 0 ? 0 : len;
    slice->capacity = slice->len;
    slice->is_owned = false;
    return (void*)slice;
}

DATARA_SYS_API void* datara_sys_slice_alloc(int64_t len) {
    if (len < 0) len = 0;
    DataraSliceView* slice = (DataraSliceView*)malloc(sizeof(DataraSliceView));
    if (!slice) return NULL;
    slice->ptr = (uint8_t*)calloc(1, len > 0 ? (size_t)len : 1);
    slice->len = len;
    slice->capacity = len;
    slice->is_owned = true;
    return (void*)slice;
}

DATARA_SYS_API void datara_sys_slice_free(void* slice_ptr) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (slice->is_owned && slice->ptr) {
        free(slice->ptr);
        slice->ptr = NULL;
    }
    free(slice);
}

DATARA_SYS_API int64_t datara_sys_slice_len(void* slice_ptr) {
    if (!slice_ptr) return 0;
    return ((DataraSliceView*)slice_ptr)->len;
}

DATARA_SYS_API int64_t datara_sys_slice_get_byte(void* slice_ptr, int64_t offset) {
    if (!slice_ptr) return 0;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset >= slice->len || !slice->ptr) return 0;
    return (int64_t)slice->ptr[offset];
}

DATARA_SYS_API void datara_sys_slice_set_byte(void* slice_ptr, int64_t offset, int64_t val) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset >= slice->len || !slice->ptr) return;
    slice->ptr[offset] = (uint8_t)(val & 0xFF);
}

DATARA_SYS_API int64_t datara_sys_slice_read_u16_be(void* slice_ptr, int64_t offset) {
    if (!slice_ptr) return 0;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 2 > slice->len || !slice->ptr) return 0;
    uint16_t val;
    memcpy(&val, slice->ptr + offset, 2);
    return (int64_t)(is_little_endian() ? BSWAP16(val) : val);
}

DATARA_SYS_API int64_t datara_sys_slice_read_u16_le(void* slice_ptr, int64_t offset) {
    if (!slice_ptr) return 0;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 2 > slice->len || !slice->ptr) return 0;
    uint16_t val;
    memcpy(&val, slice->ptr + offset, 2);
    return (int64_t)(is_little_endian() ? val : BSWAP16(val));
}

DATARA_SYS_API int64_t datara_sys_slice_read_u32_be(void* slice_ptr, int64_t offset) {
    if (!slice_ptr) return 0;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 4 > slice->len || !slice->ptr) return 0;
    uint32_t val;
    memcpy(&val, slice->ptr + offset, 4);
    return (int64_t)(is_little_endian() ? BSWAP32(val) : val);
}

DATARA_SYS_API int64_t datara_sys_slice_read_u32_le(void* slice_ptr, int64_t offset) {
    if (!slice_ptr) return 0;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 4 > slice->len || !slice->ptr) return 0;
    uint32_t val;
    memcpy(&val, slice->ptr + offset, 4);
    return (int64_t)(is_little_endian() ? val : BSWAP32(val));
}

DATARA_SYS_API int64_t datara_sys_slice_read_u64_be(void* slice_ptr, int64_t offset) {
    if (!slice_ptr) return 0;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 8 > slice->len || !slice->ptr) return 0;
    uint64_t val;
    memcpy(&val, slice->ptr + offset, 8);
    return (int64_t)(is_little_endian() ? BSWAP64(val) : val);
}

DATARA_SYS_API int64_t datara_sys_slice_read_u64_le(void* slice_ptr, int64_t offset) {
    if (!slice_ptr) return 0;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 8 > slice->len || !slice->ptr) return 0;
    uint64_t val;
    memcpy(&val, slice->ptr + offset, 8);
    return (int64_t)(is_little_endian() ? val : BSWAP64(val));
}

DATARA_SYS_API void datara_sys_slice_write_u16_be(void* slice_ptr, int64_t offset, int64_t val) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 2 > slice->len || !slice->ptr) return;
    uint16_t u = (uint16_t)val;
    uint16_t swapped = is_little_endian() ? BSWAP16(u) : u;
    memcpy(slice->ptr + offset, &swapped, 2);
}

DATARA_SYS_API void datara_sys_slice_write_u16_le(void* slice_ptr, int64_t offset, int64_t val) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 2 > slice->len || !slice->ptr) return;
    uint16_t u = (uint16_t)val;
    uint16_t swapped = is_little_endian() ? u : BSWAP16(u);
    memcpy(slice->ptr + offset, &swapped, 2);
}

DATARA_SYS_API void datara_sys_slice_write_u32_be(void* slice_ptr, int64_t offset, int64_t val) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 4 > slice->len || !slice->ptr) return;
    uint32_t u = (uint32_t)val;
    uint32_t swapped = is_little_endian() ? BSWAP32(u) : u;
    memcpy(slice->ptr + offset, &swapped, 4);
}

DATARA_SYS_API void datara_sys_slice_write_u32_le(void* slice_ptr, int64_t offset, int64_t val) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 4 > slice->len || !slice->ptr) return;
    uint32_t u = (uint32_t)val;
    uint32_t swapped = is_little_endian() ? u : BSWAP32(u);
    memcpy(slice->ptr + offset, &swapped, 4);
}

DATARA_SYS_API void datara_sys_slice_write_u64_be(void* slice_ptr, int64_t offset, int64_t val) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 8 > slice->len || !slice->ptr) return;
    uint64_t u = (uint64_t)val;
    uint64_t swapped = is_little_endian() ? BSWAP64(u) : u;
    memcpy(slice->ptr + offset, &swapped, 8);
}

DATARA_SYS_API void datara_sys_slice_write_u64_le(void* slice_ptr, int64_t offset, int64_t val) {
    if (!slice_ptr) return;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset + 8 > slice->len || !slice->ptr) return;
    uint64_t u = (uint64_t)val;
    uint64_t swapped = is_little_endian() ? u : BSWAP64(u);
    memcpy(slice->ptr + offset, &swapped, 8);
}

DATARA_SYS_API void* datara_sys_slice_subslice(void* slice_ptr, int64_t offset, int64_t len) {
    if (!slice_ptr) return NULL;
    DataraSliceView* slice = (DataraSliceView*)slice_ptr;
    if (offset < 0 || offset > slice->len) return NULL;
    if (len < 0 || offset + len > slice->len) return NULL;
    
    DataraSliceView* sub = (DataraSliceView*)malloc(sizeof(DataraSliceView));
    if (!sub) return NULL;
    sub->ptr = slice->ptr + offset;
    sub->len = len;
    sub->capacity = len;
    sub->is_owned = false;
    return (void*)sub;
}

#if defined(_MSC_VER)
  #include <windows.h>
#endif

// Level 4: Hardware MMIO & Volatile Access implementations
DATARA_SYS_API int64_t datara_hw_volatile_read8(void* ptr) {
    if (!ptr) return 0;
    return (int64_t)(*(volatile uint8_t*)ptr);
}

DATARA_SYS_API int64_t datara_hw_volatile_read16(void* ptr) {
    if (!ptr) return 0;
    return (int64_t)(*(volatile uint16_t*)ptr);
}

DATARA_SYS_API int64_t datara_hw_volatile_read32(void* ptr) {
    if (!ptr) return 0;
    return (int64_t)(*(volatile uint32_t*)ptr);
}

DATARA_SYS_API int64_t datara_hw_volatile_read64(void* ptr) {
    if (!ptr) return 0;
    return (int64_t)(*(volatile uint64_t*)ptr);
}

DATARA_SYS_API void datara_hw_volatile_write8(void* ptr, int64_t val) {
    if (!ptr) return;
    *(volatile uint8_t*)ptr = (uint8_t)val;
}

DATARA_SYS_API void datara_hw_volatile_write16(void* ptr, int64_t val) {
    if (!ptr) return;
    *(volatile uint16_t*)ptr = (uint16_t)val;
}

DATARA_SYS_API void datara_hw_volatile_write32(void* ptr, int64_t val) {
    if (!ptr) return;
    *(volatile uint32_t*)ptr = (uint32_t)val;
}

DATARA_SYS_API void datara_hw_volatile_write64(void* ptr, int64_t val) {
    if (!ptr) return;
    *(volatile uint64_t*)ptr = (uint64_t)val;
}

// Level 4: Hardware Memory Fences
DATARA_SYS_API void datara_hw_atomic_fence_acquire(void) {
#if defined(_MSC_VER)
    MemoryBarrier();
#else
    __atomic_thread_fence(__ATOMIC_ACQUIRE);
#endif
}

DATARA_SYS_API void datara_hw_atomic_fence_release(void) {
#if defined(_MSC_VER)
    MemoryBarrier();
#else
    __atomic_thread_fence(__ATOMIC_RELEASE);
#endif
}

DATARA_SYS_API void datara_hw_atomic_fence_acq_rel(void) {
#if defined(_MSC_VER)
    MemoryBarrier();
#else
    __atomic_thread_fence(__ATOMIC_ACQ_REL);
#endif
}

DATARA_SYS_API void* datara_hw_volatile_ptr(void* ptr) {
    return ptr;
}

DATARA_SYS_API void datara_hw_atomic_fence_seq_cst(void) {
#if defined(_MSC_VER)
    MemoryBarrier();
#else
    __atomic_thread_fence(__ATOMIC_SEQ_CST);
#endif
}

DATARA_SYS_API void datara_hw_atomic_fence(const char* order) {
    if (!order) {
        datara_hw_atomic_fence_seq_cst();
        return;
    }
    if (strstr(order, "Acquire") || strstr(order, "acquire")) {
        datara_hw_atomic_fence_acquire();
    } else if (strstr(order, "Release") || strstr(order, "release")) {
        datara_hw_atomic_fence_release();
    } else if (strstr(order, "AcqRel") || strstr(order, "acq_rel")) {
        datara_hw_atomic_fence_acq_rel();
    } else {
        datara_hw_atomic_fence_seq_cst();
    }
}

// Level 4: Zero Initialization (Guaranteed Zero UB)
DATARA_SYS_API void* datara_hw_typed_zero_init(int64_t size) {
    if (size <= 0) size = 8;
    return calloc(1, (size_t)size);
}
