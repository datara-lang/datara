#ifndef TEST_EXTERN_BLOCK_H
#define TEST_EXTERN_BLOCK_H

#define BLOCK_BASE 1000

/* Block form (v1.3.2 M2): several signatures listed inside an
   extern "C" linkage block, guarded for C++ consumers. */

#ifdef __cplusplus
extern "C" {
#endif

long long block_add(long long a, long long b);
long long block_scale(long long x, long long k);

#ifdef __cplusplus
}
#endif

/* Unguarded linkage block with an enum, a define and a nested-style
   declaration sequence. */
extern "C" {
enum BlockStatus {
    BLOCK_IDLE = 7,
    BLOCK_BUSY = 8
};

long long block_negate(long long v);
}

/* Legacy per-item declaration outside any linkage block must keep
   working alongside the block forms. */
long long block_plain(long long v);

#endif
