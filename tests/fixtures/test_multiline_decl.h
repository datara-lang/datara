#ifndef TEST_MULTILINE_DECL_H
#define TEST_MULTILINE_DECL_H

/* Multi-line declarations (v1.3.2 M2): return type, parameter list and
   terminating semicolon spread across several source lines. */

typedef struct {
    long long
    base;
    long long len;
} MlSpan;

long long ml_add(
    long long a,
    long long b
);

long long
ml_mul(long long a, long long b);

long long ml_fold(
    MlSpan s,
    long long factor
);

long long ml_apply(
    long long v,
    long long (*cb)(long long)
);

#endif
