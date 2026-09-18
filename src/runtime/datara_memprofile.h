#ifndef DATARA_MEMPROFILE_H
#define DATARA_MEMPROFILE_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef _WIN32
  #define DATARA_MEM_API __declspec(dllexport)
#else
  #define DATARA_MEM_API __attribute__((visibility("default")))
#endif

typedef enum {
    DATARA_MEM_TAG_HEAP = 0,
    DATARA_MEM_TAG_ARENA = 1,
    DATARA_MEM_TAG_SLAB = 2,
    DATARA_MEM_TAG_SCRATCHPAD = 3,
    DATARA_MEM_TAG_SHADOW = 4
} DataraMemTag;

typedef struct {
    uint64_t total_alloc_count;
    uint64_t total_free_count;
    uint64_t total_allocated_bytes;
    uint64_t total_freed_bytes;
    uint64_t current_live_bytes;
    uint64_t peak_live_bytes;
    uint64_t arena_bytes;
    uint64_t scratchpad_bytes;
    uint64_t promotions_count;
    uint64_t promoted_bytes;
    uint64_t slab_hits;
    uint64_t slab_misses;
} DataraMemSnapshot;

// Control
DATARA_MEM_API void datara_memprofile_start(void);
DATARA_MEM_API void datara_memprofile_stop(void);
DATARA_MEM_API bool datara_memprofile_is_active(void);
DATARA_MEM_API void datara_memprofile_reset(void);

// Tracking events
DATARA_MEM_API void datara_memprofile_record_alloc(size_t bytes, int32_t tag);
DATARA_MEM_API void datara_memprofile_record_free(size_t bytes, int32_t tag);
DATARA_MEM_API void datara_memprofile_record_promote(size_t bytes);
DATARA_MEM_API void datara_memprofile_record_slab(bool hit);

// Snapshot & Export
DATARA_MEM_API void datara_memprofile_snapshot(DataraMemSnapshot* out_snapshot);
DATARA_MEM_API const char* datara_memprofile_summary_json(void);
DATARA_MEM_API int32_t datara_memprofile_dump(const char* filepath);

#endif // DATARA_MEMPROFILE_H
