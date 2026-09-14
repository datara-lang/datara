#ifndef TEST_EXTERN_BLOCK_BAD_RET_H
#define TEST_EXTERN_BLOCK_BAD_RET_H

/* Block form containing a function whose by-value struct return exceeds
   one 64-bit machine word: must still be rejected with E0962 (v1.3.1
   soundness gate applies regardless of declaration form). */

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    long long x;
    long long y;
} BigPoint;

BigPoint big_add(BigPoint a, BigPoint b);
long long big_sum(BigPoint a, BigPoint b);

#ifdef __cplusplus
}
#endif

#endif
