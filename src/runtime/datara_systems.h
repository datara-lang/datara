#ifndef DATARA_SYSTEMS_H
#define DATARA_SYSTEMS_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef _WIN32
  #define DATARA_SYS_API __declspec(dllexport)
#else
  #define DATARA_SYS_API __attribute__((visibility("default")))
#endif

typedef struct {
    uint8_t* ptr;
    int64_t len;
    int64_t capacity;
    bool is_owned;
} DataraSliceView;

// Endian conversion intrinsics (network & binary protocols)
DATARA_SYS_API int64_t datara_sys_hton16(int64_t val);
DATARA_SYS_API int64_t datara_sys_ntoh16(int64_t val);
DATARA_SYS_API int64_t datara_sys_hton32(int64_t val);
DATARA_SYS_API int64_t datara_sys_ntoh32(int64_t val);
DATARA_SYS_API int64_t datara_sys_hton64(int64_t val);
DATARA_SYS_API int64_t datara_sys_ntoh64(int64_t val);
DATARA_SYS_API int64_t datara_sys_bswap16(int64_t val);
DATARA_SYS_API int64_t datara_sys_bswap32(int64_t val);
DATARA_SYS_API int64_t datara_sys_bswap64(int64_t val);

// Zero-copy SliceView operations
DATARA_SYS_API void* datara_sys_slice_from_buffer(void* ptr, int64_t len);
DATARA_SYS_API void* datara_sys_slice_alloc(int64_t len);
DATARA_SYS_API void datara_sys_slice_free(void* slice_ptr);
DATARA_SYS_API int64_t datara_sys_slice_len(void* slice_ptr);
DATARA_SYS_API int64_t datara_sys_slice_get_byte(void* slice_ptr, int64_t offset);
DATARA_SYS_API void datara_sys_slice_set_byte(void* slice_ptr, int64_t offset, int64_t val);
DATARA_SYS_API int64_t datara_sys_slice_read_u16_be(void* slice_ptr, int64_t offset);
DATARA_SYS_API int64_t datara_sys_slice_read_u16_le(void* slice_ptr, int64_t offset);
DATARA_SYS_API int64_t datara_sys_slice_read_u32_be(void* slice_ptr, int64_t offset);
DATARA_SYS_API int64_t datara_sys_slice_read_u32_le(void* slice_ptr, int64_t offset);
DATARA_SYS_API int64_t datara_sys_slice_read_u64_be(void* slice_ptr, int64_t offset);
DATARA_SYS_API int64_t datara_sys_slice_read_u64_le(void* slice_ptr, int64_t offset);
DATARA_SYS_API void datara_sys_slice_write_u16_be(void* slice_ptr, int64_t offset, int64_t val);
DATARA_SYS_API void datara_sys_slice_write_u16_le(void* slice_ptr, int64_t offset, int64_t val);
DATARA_SYS_API void datara_sys_slice_write_u32_be(void* slice_ptr, int64_t offset, int64_t val);
DATARA_SYS_API void datara_sys_slice_write_u32_le(void* slice_ptr, int64_t offset, int64_t val);
DATARA_SYS_API void datara_sys_slice_write_u64_be(void* slice_ptr, int64_t offset, int64_t val);
DATARA_SYS_API void datara_sys_slice_write_u64_le(void* slice_ptr, int64_t offset, int64_t val);
DATARA_SYS_API void* datara_sys_slice_subslice(void* slice_ptr, int64_t offset, int64_t len);

// Level 4: Hardware MMIO & Volatile Access
DATARA_SYS_API int64_t datara_hw_volatile_read8(void* ptr);
DATARA_SYS_API int64_t datara_hw_volatile_read16(void* ptr);
DATARA_SYS_API int64_t datara_hw_volatile_read32(void* ptr);
DATARA_SYS_API int64_t datara_hw_volatile_read64(void* ptr);
DATARA_SYS_API void datara_hw_volatile_write8(void* ptr, int64_t val);
DATARA_SYS_API void datara_hw_volatile_write16(void* ptr, int64_t val);
DATARA_SYS_API void datara_hw_volatile_write32(void* ptr, int64_t val);
DATARA_SYS_API void datara_hw_volatile_write64(void* ptr, int64_t val);

DATARA_SYS_API void* datara_hw_volatile_ptr(void* ptr);

// Level 4: Hardware Memory Fences
DATARA_SYS_API void datara_hw_atomic_fence_acquire(void);
DATARA_SYS_API void datara_hw_atomic_fence_release(void);
DATARA_SYS_API void datara_hw_atomic_fence_acq_rel(void);
DATARA_SYS_API void datara_hw_atomic_fence_seq_cst(void);
DATARA_SYS_API void datara_hw_atomic_fence(const char* order);

// Level 4: Zero Initialization (Guaranteed Zero UB)
DATARA_SYS_API void* datara_hw_typed_zero_init(int64_t size);

#endif // DATARA_SYSTEMS_H
