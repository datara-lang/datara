#ifndef DATARA_RT_INTERNAL_H
#define DATARA_RT_INTERNAL_H

/* Internal cross-translation-unit contract for the split Datara runtime.
 *
 * P4: datara_runtime.c was a 7.7k-line / 265KB monolith; it is now split into
 * per-subsystem translation units. The public ABI stays in datara_runtime.h —
 * this header exposes only what the split modules genuinely share and is NOT
 * part of the public ABI.
 */

#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "datara_runtime.h"

static inline void datara_fast_copy(char* dst, const char* src, size_t len) {
    if (len <= 8) {
        uint64_t v = 0;
        memcpy(&v, src, len);
        memcpy(dst, &v, len);
    } else if (len <= 16) {
        uint64_t v1, v2;
        memcpy(&v1, src, 8);
        memcpy(&v2, src + len - 8, 8);
        memcpy(dst, &v1, 8);
        memcpy(dst + len - 8, &v2, 8);
    } else {
        memcpy(dst, src, len);
    }
}

#if defined(_MSC_VER)
#define DATARA_TLS __declspec(thread)
#else
#define DATARA_TLS __thread
#endif

/* ------------------------------------------------------------------ *
 * Allocation counters (defined in datara_alloc.c; incremented by
 * callers that fall back to raw malloc inside their own growth paths)
 * ------------------------------------------------------------------ */
extern DATARA_TLS int64_t tls_heap_alloc_count;

/* ------------------------------------------------------------------ *
 * List header layout (defined here because datara_mem.c frees lists
 * owned by datara_list.c)
 * ------------------------------------------------------------------ */
typedef struct {
    int64_t capacity;
    int64_t magic;
} DataraListHeader;

#define DATARA_LIST_MAGIC      0x4441544C49535430ULL
#define DATARA_LIST_MAGIC_MASK 0xFFFFFFFFFFFFFFF0ULL
#define DATARA_LIST_FLAG_HEAP  0x0ULL
#define DATARA_LIST_FLAG_POOL  0x1ULL
#define DATARA_LIST_FLAG_STACK 0x2ULL
#define DATARA_LIST_FLAG_SMALL 0x4ULL

/* ------------------------------------------------------------------ *
 * Scratch string ring (datara_string.c)
 * ------------------------------------------------------------------ */
#define DATARA_SCRATCH_RING_SIZE (1024 * 1024)
char* datara_str_scratch_alloc(size_t len);
int   datara_str_in_scratch_ring(const void* p);

/* Fast decimal formatter (datara_string.c; used by datara_map.c) */
size_t fast_i64toa(int64_t val, char* buf);

/* ------------------------------------------------------------------ *
 * Outcome builder (datara_fs.c; used by the net/ui/fs modules)
 * ------------------------------------------------------------------ */
void* datara_rt_outcome_build(int64_t is_success, const char* value, const char* msg);
void* datara_rt_outcome_build_raw(int64_t is_success, int64_t value_raw, const char* msg);

/* ------------------------------------------------------------------ *
 * RNG insecurity flag (defined in datara_crypto.c; set when
 * datara_rt_random_bytes fell back to the clock-seeded LCG)
 * ------------------------------------------------------------------ */
extern int g_datara_rng_insecure;

/* ------------------------------------------------------------------ *
 * PGO auto-flush hook (datara_pgo.c; called from datara_rt_exit)
 * ------------------------------------------------------------------ */
void datara_rt_pgo_auto_flush(void);

#endif /* DATARA_RT_INTERNAL_H */
