#if defined(__APPLE__)
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE 1
#endif
#else
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <errno.h>
#include "datara_runtime.h"
#include "datara_rt_internal.h"

#if defined(_MSC_VER) && !defined(__clang__)
#include <intrin.h>
#endif

// ---------------------------------------------------------------------------
// High-Performance Fast Math
// ---------------------------------------------------------------------------
double datara_rt_math_sqrt(double x) { return sqrt(x); }
double datara_rt_math_pow(double base, double exp) { return pow(base, exp); }
double datara_rt_math_abs(double x) { return fabs(x); }
double datara_rt_math_sin(double x) { return sin(x); }
double datara_rt_math_cos(double x) { return cos(x); }
double datara_rt_math_tan(double x) { return tan(x); }
double datara_rt_math_floor(double x) { return floor(x); }
double datara_rt_math_ceil(double x) { return ceil(x); }
double datara_rt_math_round(double x) { return round(x); }
double datara_rt_math_min(double a, double b) { return fmin(a, b); }
double datara_rt_math_max(double a, double b) { return fmax(a, b); }
double datara_rt_math_clamp(double val, double min_val, double max_val) {
    if (val < min_val) return min_val;
    if (val > max_val) return max_val;
    return val;
}
double datara_rt_math_hypot(double a, double b) { return hypot(a, b); }
double datara_rt_math_log(double x) { return log(x); }
double datara_rt_math_exp(double x) { return exp(x); }
int64_t datara_rt_math_min_int(int64_t a, int64_t b) { return a < b ? a : b; }
int64_t datara_rt_math_max_int(int64_t a, int64_t b) { return a > b ? a : b; }
int64_t datara_rt_math_clamp_int(int64_t val, int64_t min_val, int64_t max_val) {
    if (val < min_val) return min_val;
    if (val > max_val) return max_val;
    return val;
}
int64_t datara_rt_math_abs_int(int64_t x) { return x < 0 ? -x : x; }
int64_t datara_rt_math_ctz(int64_t v) {
    if (v == 0) return 64;
#if defined(_MSC_VER) && !defined(__clang__)
    unsigned long idx;
    _BitScanForward64(&idx, (unsigned __int64)v);
    return (int64_t)idx;
#else
    return (int64_t)__builtin_ctzll((unsigned long long)v);
#endif
}
int64_t datara_rt_math_shr(int64_t v, int64_t s) { return v >> (s & 63); }
int64_t datara_rt_math_shl(int64_t v, int64_t s) { return (int64_t)((uint64_t)v << (s & 63)); }
int64_t datara_rt_math_xor(int64_t a, int64_t b) { return a ^ b; }
int64_t datara_rt_math_and(int64_t a, int64_t b) { return a & b; }
int64_t datara_rt_math_or(int64_t a, int64_t b) { return a | b; }
