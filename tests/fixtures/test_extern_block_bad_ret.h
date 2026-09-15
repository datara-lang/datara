#ifndef TEST_EXTERN_BLOCK_BAD_RET_H
#define TEST_EXTERN_BLOCK_BAD_RET_H

/* Block-form declarations exercising the struct-return ABI boundary.
   Since v1.3.2 M3 the layout-compatible BigPoint return takes the
   hidden sret slot; the layout-incompatible MixedPoint return keeps
   the E0962 compile-time rejection regardless of declaration form. */

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    long long x;
    long long y;
} BigPoint;

/* Fields land at offsets 0/4/8 while Datara reads at 0/8/16: never
   eligible for the sret path. */
typedef struct {
    int a;
    int b;
    long long c;
} MixedPoint;

BigPoint big_add(BigPoint a, BigPoint b);
long long big_sum(BigPoint a, BigPoint b);
MixedPoint big_mixed(void);

#ifdef __cplusplus
}
#endif

#endif
