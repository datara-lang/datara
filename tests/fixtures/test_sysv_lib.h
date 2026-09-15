#ifndef TEST_SYSV_LIB_H
#define TEST_SYSV_LIB_H

/* v1.3.3 SysV AMD64 register-pair struct-return fixture (linux-x86-64).
   Every function below takes SCALARS only and returns a 16-byte
   layout-compatible struct: the two eightbytes ride the SysV register
   pairs (INTEGER -> RAX then RDX, SSE -> XMM0 then XMM1, independent
   per-class sequences). Struct-by-value PARAMETERS are intentionally
   absent: they are a separate ABI surface outside the v1.3.3 scope. */

/* {long long, long long}: INTEGER + INTEGER -> RAX:RDX. */
typedef struct {
    long long a;
    long long b;
} I64Pair;

/* {double, double}: SSE + SSE -> XMM0:XMM1. */
typedef struct {
    double x;
    double y;
} F64Pair;

/* {long long, double}: INTEGER + SSE -> RAX:XMM0. */
typedef struct {
    long long a;
    double x;
} MixedIF;

/* {double, long long}: SSE + INTEGER -> XMM0:RAX.
   Pins the per-class register sequences: the INTEGER eightbyte rides
   RAX even though it is the second eightbyte of the aggregate. */
typedef struct {
    double x;
    long long a;
} MixedFI;

/* 8 bytes: returned in RAX, the pre-existing register path. */
typedef struct {
    long long v;
} Word;

/* Fields land at offsets 0/4/8 while Datara reads at 0/8/16: never
   layout-compatible, rejected with E0962 on every target. */
typedef struct {
    int a;
    int b;
    long long c;
} Wide3;

/* 24 bytes: three INTEGER eightbytes, above the two-eightbyte window.
   Rejected with E0962 on every target. */
typedef struct {
    long long a;
    long long b;
    long long c;
} Triple;

I64Pair make_i64_pair(long long a, long long b);
F64Pair make_f64_pair(double x, double y);
MixedIF make_mixed_if(long long a, double x);
MixedFI make_mixed_fi(double x, long long a);

Word make_word(long long v);
/* 8-byte struct-by-value parameter: SysV passes it as one INTEGER in RDI,
   which is exactly the raw word Datara holds, so this consumer works on
   the unchanged register path. (>8-byte by-value parameters are a
   different ABI surface, deliberately absent here.) */
long long word_value(Word w);

/* Variadic struct return: the import path does not model the variadic
   ABI state (AL holds the SSE register count on System V), so this stays
   rejected with E0962 even on SysV targets. */
I64Pair make_i64_pair_variadic(long long a, ...);

Wide3 make_wide3(int a, int b, long long c);
Triple make_triple(long long a, long long b, long long c);

#endif
