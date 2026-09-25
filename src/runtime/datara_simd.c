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

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#include <dbghelp.h>
#else
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/mman.h>
#ifndef MAP_ANONYMOUS
#ifdef MAP_ANON
#define MAP_ANONYMOUS MAP_ANON
#endif
#endif
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <unistd.h>
#include <fcntl.h>
#include <time.h>
#include <pthread.h>
#include <sched.h>
#include <dirent.h>
#if defined(__has_include)
#if __has_include(<execinfo.h>)
#include <execinfo.h>
#define DATARA_HAS_EXECINFO 1
#endif
#elif !defined(__musl__)
#include <execinfo.h>
#define DATARA_HAS_EXECINFO 1
#endif
#endif

#ifndef INADDR_NONE
#define INADDR_NONE ((unsigned long)0xffffffff)
#endif
// ---------------------------------------------------------------------------
// First-Class SIMD 4D Vector Math
// ---------------------------------------------------------------------------
// SSE2 intrinsics are used for the hot vector operations when the target is
// x86-64 / SSE2-capable. Everything falls back to a portable scalar path
// otherwise, and datara_rt_simd_enabled() reports which path is compiled in.
#if defined(__x86_64__) || defined(_M_X64) || defined(__SSE2__) || (defined(_MSC_VER) && !defined(_M_ARM64))
#define DATARA_SIMD_SSE2 1
#include <emmintrin.h>
#endif

int datara_rt_simd_enabled(void) {
#if defined(DATARA_SIMD_SSE2)
    return 1;
#else
    return 0;
#endif
}

DataraFloat4 datara_rt_float4(double x, double y, double z, double w) {
    DataraFloat4 v;
    v.x = (float)x; v.y = (float)y; v.z = (float)z; v.w = (float)w;
    return v;
}

DataraInt4 datara_rt_int4(int64_t x, int64_t y, int64_t z, int64_t w) {
    DataraInt4 v;
    v.x = (int32_t)x; v.y = (int32_t)y; v.z = (int32_t)z; v.w = (int32_t)w;
    return v;
}

double datara_rt_float4_dot(DataraFloat4 a, DataraFloat4 b) {
#if defined(DATARA_SIMD_SSE2)
    /* One packed multiply + SSE2 shuffle/add horizontal reduction. */
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 vb = _mm_loadu_ps(&b.x);
    __m128 prod = _mm_mul_ps(va, vb);
    /* Round 1: add lanes (1,0,3,2) -> partial sums in both low lanes. */
    __m128 shuf = _mm_shuffle_ps(prod, prod, _MM_SHUFFLE(2, 3, 0, 1));
    __m128 sums = _mm_add_ps(prod, shuf);
    /* Round 2: add lanes (2,3,0,1) -> full sum broadcast to all lanes. */
    shuf = _mm_shuffle_ps(sums, sums, _MM_SHUFFLE(1, 0, 3, 2));
    sums = _mm_add_ps(sums, shuf);
    return (double)_mm_cvtss_f32(sums);
#else
    return (double)(a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w);
#endif
}

DataraFloat4 datara_rt_float4_min4(DataraFloat4 a, DataraFloat4 b) {
    DataraFloat4 out;
#if defined(DATARA_SIMD_SSE2)
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 vb = _mm_loadu_ps(&b.x);
    _mm_storeu_ps(&out.x, _mm_min_ps(va, vb));
#else
    out.x = a.x < b.x ? a.x : b.x;
    out.y = a.y < b.y ? a.y : b.y;
    out.z = a.z < b.z ? a.z : b.z;
    out.w = a.w < b.w ? a.w : b.w;
#endif
    return out;
}

DataraFloat4 datara_rt_float4_max4(DataraFloat4 a, DataraFloat4 b) {
    DataraFloat4 out;
#if defined(DATARA_SIMD_SSE2)
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 vb = _mm_loadu_ps(&b.x);
    _mm_storeu_ps(&out.x, _mm_max_ps(va, vb));
#else
    out.x = a.x > b.x ? a.x : b.x;
    out.y = a.y > b.y ? a.y : b.y;
    out.z = a.z > b.z ? a.z : b.z;
    out.w = a.w > b.w ? a.w : b.w;
#endif
    return out;
}

DataraInt4 datara_rt_int4_min4(DataraInt4 a, DataraInt4 b) {
    DataraInt4 out;
#if defined(DATARA_SIMD_SSE2)
    /* SSE2 has no integer min/max for 32-bit lanes: compare, then blend
       with and/andnot (select a where a > b for min, else b). */
    __m128i va = _mm_loadu_si128((const __m128i*)&a.x);
    __m128i vb = _mm_loadu_si128((const __m128i*)&b.x);
    __m128i gt = _mm_cmpgt_epi32(va, vb);
    _mm_storeu_si128((__m128i*)&out.x,
                     _mm_or_si128(_mm_and_si128(gt, vb), _mm_andnot_si128(gt, va)));
#else
    out.x = a.x < b.x ? a.x : b.x;
    out.y = a.y < b.y ? a.y : b.y;
    out.z = a.z < b.z ? a.z : b.z;
    out.w = a.w < b.w ? a.w : b.w;
#endif
    return out;
}

DataraInt4 datara_rt_int4_max4(DataraInt4 a, DataraInt4 b) {
    DataraInt4 out;
#if defined(DATARA_SIMD_SSE2)
    __m128i va = _mm_loadu_si128((const __m128i*)&a.x);
    __m128i vb = _mm_loadu_si128((const __m128i*)&b.x);
    __m128i gt = _mm_cmpgt_epi32(va, vb);
    _mm_storeu_si128((__m128i*)&out.x,
                     _mm_or_si128(_mm_and_si128(gt, va), _mm_andnot_si128(gt, vb)));
#else
    out.x = a.x > b.x ? a.x : b.x;
    out.y = a.y > b.y ? a.y : b.y;
    out.z = a.z > b.z ? a.z : b.z;
    out.w = a.w > b.w ? a.w : b.w;
#endif
    return out;
}

// ---------------------------------------------------------------------------
// std.simd implementations (Phase 2)
// ---------------------------------------------------------------------------

DataraF32x4 datara_rt_f32x4(double x, double y, double z, double w) {
    return datara_rt_float4(x, y, z, w);
}

DataraF32x8 datara_rt_f32x8(double v0, double v1, double v2, double v3, double v4, double v5, double v6, double v7) {
    DataraF32x8 out;
    out.v[0] = (float)v0; out.v[1] = (float)v1; out.v[2] = (float)v2; out.v[3] = (float)v3;
    out.v[4] = (float)v4; out.v[5] = (float)v5; out.v[6] = (float)v6; out.v[7] = (float)v7;
    return out;
}

DataraF32x16 datara_rt_f32x16(double v0, double v1, double v2, double v3, double v4, double v5, double v6, double v7,
                              double v8, double v9, double v10, double v11, double v12, double v13, double v14, double v15) {
    DataraF32x16 out;
    out.v[0] = (float)v0; out.v[1] = (float)v1; out.v[2] = (float)v2; out.v[3] = (float)v3;
    out.v[4] = (float)v4; out.v[5] = (float)v5; out.v[6] = (float)v6; out.v[7] = (float)v7;
    out.v[8] = (float)v8; out.v[9] = (float)v9; out.v[10] = (float)v10; out.v[11] = (float)v11;
    out.v[12] = (float)v12; out.v[13] = (float)v13; out.v[14] = (float)v14; out.v[15] = (float)v15;
    return out;
}

DataraI32x4 datara_rt_i32x4(int64_t x, int64_t y, int64_t z, int64_t w) {
    return datara_rt_int4(x, y, z, w);
}

DataraI32x8 datara_rt_i32x8(int64_t v0, int64_t v1, int64_t v2, int64_t v3, int64_t v4, int64_t v5, int64_t v6, int64_t v7) {
    DataraI32x8 out;
    out.v[0] = (int32_t)v0; out.v[1] = (int32_t)v1; out.v[2] = (int32_t)v2; out.v[3] = (int32_t)v3;
    out.v[4] = (int32_t)v4; out.v[5] = (int32_t)v5; out.v[6] = (int32_t)v6; out.v[7] = (int32_t)v7;
    return out;
}

DataraF64x2 datara_rt_f64x2(double x, double y) {
    DataraF64x2 out;
    out.x = x; out.y = y;
    return out;
}

DataraF64x4 datara_rt_f64x4(double x, double y, double z, double w) {
    DataraF64x4 out;
    out.x = x; out.y = y; out.z = z; out.w = w;
    return out;
}

DataraF32x4 datara_rt_f32x4_add(DataraF32x4 a, DataraF32x4 b) {
    DataraF32x4 out;
#if defined(DATARA_SIMD_SSE2)
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 vb = _mm_loadu_ps(&b.x);
    _mm_storeu_ps(&out.x, _mm_add_ps(va, vb));
#else
    out.x = a.x + b.x; out.y = a.y + b.y; out.z = a.z + b.z; out.w = a.w + b.w;
#endif
    return out;
}

DataraF32x4 datara_rt_f32x4_sub(DataraF32x4 a, DataraF32x4 b) {
    DataraF32x4 out;
#if defined(DATARA_SIMD_SSE2)
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 vb = _mm_loadu_ps(&b.x);
    _mm_storeu_ps(&out.x, _mm_sub_ps(va, vb));
#else
    out.x = a.x - b.x; out.y = a.y - b.y; out.z = a.z - b.z; out.w = a.w - b.w;
#endif
    return out;
}

DataraF32x4 datara_rt_f32x4_mul(DataraF32x4 a, DataraF32x4 b) {
    DataraF32x4 out;
#if defined(DATARA_SIMD_SSE2)
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 vb = _mm_loadu_ps(&b.x);
    _mm_storeu_ps(&out.x, _mm_mul_ps(va, vb));
#else
    out.x = a.x * b.x; out.y = a.y * b.y; out.z = a.z * b.z; out.w = a.w * b.w;
#endif
    return out;
}

DataraF32x4 datara_rt_f32x4_div(DataraF32x4 a, DataraF32x4 b) {
    DataraF32x4 out;
#if defined(DATARA_SIMD_SSE2)
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 vb = _mm_loadu_ps(&b.x);
    _mm_storeu_ps(&out.x, _mm_div_ps(va, vb));
#else
    out.x = a.x / b.x; out.y = a.y / b.y; out.z = a.z / b.z; out.w = a.w / b.w;
#endif
    return out;
}

double datara_rt_f32x4_dot(DataraF32x4 a, DataraF32x4 b) {
    return datara_rt_float4_dot(a, b);
}

DataraF32x4 datara_rt_f32x4_cross(DataraF32x4 a, DataraF32x4 b) {
    DataraF32x4 out;
    out.x = a.y * b.z - a.z * b.y;
    out.y = a.z * b.x - a.x * b.z;
    out.z = a.x * b.y - a.y * b.x;
    out.w = 0.0f;
    return out;
}

double datara_rt_f32x4_horizontal_add(DataraF32x4 a) {
#if defined(DATARA_SIMD_SSE2)
    __m128 va = _mm_loadu_ps(&a.x);
    __m128 shuf = _mm_shuffle_ps(va, va, _MM_SHUFFLE(2, 3, 0, 1));
    __m128 sums = _mm_add_ps(va, shuf);
    shuf = _mm_shuffle_ps(sums, sums, _MM_SHUFFLE(1, 0, 3, 2));
    sums = _mm_add_ps(sums, shuf);
    return (double)_mm_cvtss_f32(sums);
#else
    return (double)(a.x + a.y + a.z + a.w);
#endif
}

DataraF32x4 datara_rt_f32x4_min(DataraF32x4 a, DataraF32x4 b) {
    return datara_rt_float4_min4(a, b);
}

DataraF32x4 datara_rt_f32x4_max(DataraF32x4 a, DataraF32x4 b) {
    return datara_rt_float4_max4(a, b);
}

DataraF32x4 datara_rt_f32x4_lerp(DataraF32x4 a, DataraF32x4 b, double t) {
    float tf = (float)t;
    DataraF32x4 out;
    out.x = a.x + tf * (b.x - a.x);
    out.y = a.y + tf * (b.y - a.y);
    out.z = a.z + tf * (b.z - a.z);
    out.w = a.w + tf * (b.w - a.w);
    return out;
}

DataraF32x4 datara_rt_f32x4_normalize(DataraF32x4 a) {
    double d = datara_rt_float4_dot(a, a);
    DataraF32x4 out;
    if (d <= 0.0) {
        out.x = 0.0f; out.y = 0.0f; out.z = 0.0f; out.w = 0.0f;
        return out;
    }
    float inv = (float)(1.0 / sqrt(d));
    out.x = a.x * inv;
    out.y = a.y * inv;
    out.z = a.z * inv;
    out.w = a.w * inv;
    return out;
}

double datara_rt_f32x4_distance(DataraF32x4 a, DataraF32x4 b) {
    DataraF32x4 diff = datara_rt_f32x4_sub(a, b);
    return sqrt(datara_rt_f32x4_dot(diff, diff));
}

// f32x8 operations
DataraF32x8 datara_rt_f32x8_add(DataraF32x8 a, DataraF32x8 b) {
    DataraF32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] + b.v[i];
    return out;
}

DataraF32x8 datara_rt_f32x8_sub(DataraF32x8 a, DataraF32x8 b) {
    DataraF32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] - b.v[i];
    return out;
}

DataraF32x8 datara_rt_f32x8_mul(DataraF32x8 a, DataraF32x8 b) {
    DataraF32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] * b.v[i];
    return out;
}

DataraF32x8 datara_rt_f32x8_div(DataraF32x8 a, DataraF32x8 b) {
    DataraF32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] / b.v[i];
    return out;
}

double datara_rt_f32x8_dot(DataraF32x8 a, DataraF32x8 b) {
    double s = 0.0;
    for (int i = 0; i < 8; i++) s += (double)(a.v[i] * b.v[i]);
    return s;
}

double datara_rt_f32x8_horizontal_add(DataraF32x8 a) {
    double s = 0.0;
    for (int i = 0; i < 8; i++) s += (double)a.v[i];
    return s;
}

DataraF32x8 datara_rt_f32x8_min(DataraF32x8 a, DataraF32x8 b) {
    DataraF32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] < b.v[i] ? a.v[i] : b.v[i];
    return out;
}

DataraF32x8 datara_rt_f32x8_max(DataraF32x8 a, DataraF32x8 b) {
    DataraF32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] > b.v[i] ? a.v[i] : b.v[i];
    return out;
}

DataraF32x8 datara_rt_f32x8_lerp(DataraF32x8 a, DataraF32x8 b, double t) {
    float tf = (float)t;
    DataraF32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] + tf * (b.v[i] - a.v[i]);
    return out;
}

DataraF32x8 datara_rt_f32x8_normalize(DataraF32x8 a) {
    double d = datara_rt_f32x8_dot(a, a);
    DataraF32x8 out;
    if (d <= 0.0) {
        for (int i = 0; i < 8; i++) out.v[i] = 0.0f;
        return out;
    }
    float inv = (float)(1.0 / sqrt(d));
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] * inv;
    return out;
}

double datara_rt_f32x8_distance(DataraF32x8 a, DataraF32x8 b) {
    DataraF32x8 diff = datara_rt_f32x8_sub(a, b);
    return sqrt(datara_rt_f32x8_dot(diff, diff));
}

// f32x16 operations
DataraF32x16 datara_rt_f32x16_add(DataraF32x16 a, DataraF32x16 b) {
    DataraF32x16 out;
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] + b.v[i];
    return out;
}

DataraF32x16 datara_rt_f32x16_sub(DataraF32x16 a, DataraF32x16 b) {
    DataraF32x16 out;
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] - b.v[i];
    return out;
}

DataraF32x16 datara_rt_f32x16_mul(DataraF32x16 a, DataraF32x16 b) {
    DataraF32x16 out;
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] * b.v[i];
    return out;
}

DataraF32x16 datara_rt_f32x16_div(DataraF32x16 a, DataraF32x16 b) {
    DataraF32x16 out;
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] / b.v[i];
    return out;
}

double datara_rt_f32x16_dot(DataraF32x16 a, DataraF32x16 b) {
    double s = 0.0;
    for (int i = 0; i < 16; i++) s += (double)(a.v[i] * b.v[i]);
    return s;
}

double datara_rt_f32x16_horizontal_add(DataraF32x16 a) {
    double s = 0.0;
    for (int i = 0; i < 16; i++) s += (double)a.v[i];
    return s;
}

DataraF32x16 datara_rt_f32x16_min(DataraF32x16 a, DataraF32x16 b) {
    DataraF32x16 out;
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] < b.v[i] ? a.v[i] : b.v[i];
    return out;
}

DataraF32x16 datara_rt_f32x16_max(DataraF32x16 a, DataraF32x16 b) {
    DataraF32x16 out;
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] > b.v[i] ? a.v[i] : b.v[i];
    return out;
}

DataraF32x16 datara_rt_f32x16_lerp(DataraF32x16 a, DataraF32x16 b, double t) {
    float tf = (float)t;
    DataraF32x16 out;
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] + tf * (b.v[i] - a.v[i]);
    return out;
}

DataraF32x16 datara_rt_f32x16_normalize(DataraF32x16 a) {
    double d = datara_rt_f32x16_dot(a, a);
    DataraF32x16 out;
    if (d <= 0.0) {
        for (int i = 0; i < 16; i++) out.v[i] = 0.0f;
        return out;
    }
    float inv = (float)(1.0 / sqrt(d));
    for (int i = 0; i < 16; i++) out.v[i] = a.v[i] * inv;
    return out;
}

double datara_rt_f32x16_distance(DataraF32x16 a, DataraF32x16 b) {
    DataraF32x16 diff = datara_rt_f32x16_sub(a, b);
    return sqrt(datara_rt_f32x16_dot(diff, diff));
}

// i32x4 operations
DataraI32x4 datara_rt_i32x4_add(DataraI32x4 a, DataraI32x4 b) {
    DataraI32x4 out;
    out.x = a.x + b.x; out.y = a.y + b.y; out.z = a.z + b.z; out.w = a.w + b.w;
    return out;
}

DataraI32x4 datara_rt_i32x4_sub(DataraI32x4 a, DataraI32x4 b) {
    DataraI32x4 out;
    out.x = a.x - b.x; out.y = a.y - b.y; out.z = a.z - b.z; out.w = a.w - b.w;
    return out;
}

DataraI32x4 datara_rt_i32x4_mul(DataraI32x4 a, DataraI32x4 b) {
    DataraI32x4 out;
    out.x = a.x * b.x; out.y = a.y * b.y; out.z = a.z * b.z; out.w = a.w * b.w;
    return out;
}

DataraI32x4 datara_rt_i32x4_div(DataraI32x4 a, DataraI32x4 b) {
    DataraI32x4 out;
    out.x = b.x != 0 ? a.x / b.x : 0;
    out.y = b.y != 0 ? a.y / b.y : 0;
    out.z = b.z != 0 ? a.z / b.z : 0;
    out.w = b.w != 0 ? a.w / b.w : 0;
    return out;
}

int64_t datara_rt_i32x4_dot(DataraI32x4 a, DataraI32x4 b) {
    return (int64_t)a.x * b.x + (int64_t)a.y * b.y + (int64_t)a.z * b.z + (int64_t)a.w * b.w;
}

int64_t datara_rt_i32x4_horizontal_add(DataraI32x4 a) {
    return (int64_t)a.x + (int64_t)a.y + (int64_t)a.z + (int64_t)a.w;
}

DataraI32x4 datara_rt_i32x4_min(DataraI32x4 a, DataraI32x4 b) {
    return datara_rt_int4_min4(a, b);
}

DataraI32x4 datara_rt_i32x4_max(DataraI32x4 a, DataraI32x4 b) {
    return datara_rt_int4_max4(a, b);
}

// i32x8 operations
DataraI32x8 datara_rt_i32x8_add(DataraI32x8 a, DataraI32x8 b) {
    DataraI32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] + b.v[i];
    return out;
}

DataraI32x8 datara_rt_i32x8_sub(DataraI32x8 a, DataraI32x8 b) {
    DataraI32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] - b.v[i];
    return out;
}

DataraI32x8 datara_rt_i32x8_mul(DataraI32x8 a, DataraI32x8 b) {
    DataraI32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] * b.v[i];
    return out;
}

DataraI32x8 datara_rt_i32x8_div(DataraI32x8 a, DataraI32x8 b) {
    DataraI32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = b.v[i] != 0 ? a.v[i] / b.v[i] : 0;
    return out;
}

int64_t datara_rt_i32x8_dot(DataraI32x8 a, DataraI32x8 b) {
    int64_t s = 0;
    for (int i = 0; i < 8; i++) s += (int64_t)a.v[i] * b.v[i];
    return s;
}

int64_t datara_rt_i32x8_horizontal_add(DataraI32x8 a) {
    int64_t s = 0;
    for (int i = 0; i < 8; i++) s += (int64_t)a.v[i];
    return s;
}

DataraI32x8 datara_rt_i32x8_min(DataraI32x8 a, DataraI32x8 b) {
    DataraI32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] < b.v[i] ? a.v[i] : b.v[i];
    return out;
}

DataraI32x8 datara_rt_i32x8_max(DataraI32x8 a, DataraI32x8 b) {
    DataraI32x8 out;
    for (int i = 0; i < 8; i++) out.v[i] = a.v[i] > b.v[i] ? a.v[i] : b.v[i];
    return out;
}

// f64x2 operations
DataraF64x2 datara_rt_f64x2_add(DataraF64x2 a, DataraF64x2 b) {
    DataraF64x2 out;
    out.x = a.x + b.x; out.y = a.y + b.y;
    return out;
}

DataraF64x2 datara_rt_f64x2_sub(DataraF64x2 a, DataraF64x2 b) {
    DataraF64x2 out;
    out.x = a.x - b.x; out.y = a.y - b.y;
    return out;
}

DataraF64x2 datara_rt_f64x2_mul(DataraF64x2 a, DataraF64x2 b) {
    DataraF64x2 out;
    out.x = a.x * b.x; out.y = a.y * b.y;
    return out;
}

DataraF64x2 datara_rt_f64x2_div(DataraF64x2 a, DataraF64x2 b) {
    DataraF64x2 out;
    out.x = a.x / b.x; out.y = a.y / b.y;
    return out;
}

double datara_rt_f64x2_dot(DataraF64x2 a, DataraF64x2 b) {
    return a.x * b.x + a.y * b.y;
}

double datara_rt_f64x2_horizontal_add(DataraF64x2 a) {
    return a.x + a.y;
}

DataraF64x2 datara_rt_f64x2_min(DataraF64x2 a, DataraF64x2 b) {
    DataraF64x2 out;
    out.x = a.x < b.x ? a.x : b.x;
    out.y = a.y < b.y ? a.y : b.y;
    return out;
}

DataraF64x2 datara_rt_f64x2_max(DataraF64x2 a, DataraF64x2 b) {
    DataraF64x2 out;
    out.x = a.x > b.x ? a.x : b.x;
    out.y = a.y > b.y ? a.y : b.y;
    return out;
}

DataraF64x2 datara_rt_f64x2_lerp(DataraF64x2 a, DataraF64x2 b, double t) {
    DataraF64x2 out;
    out.x = a.x + t * (b.x - a.x);
    out.y = a.y + t * (b.y - a.y);
    return out;
}

DataraF64x2 datara_rt_f64x2_normalize(DataraF64x2 a) {
    double d = datara_rt_f64x2_dot(a, a);
    DataraF64x2 out;
    if (d <= 0.0) {
        out.x = 0.0; out.y = 0.0;
        return out;
    }
    double inv = 1.0 / sqrt(d);
    out.x = a.x * inv;
    out.y = a.y * inv;
    return out;
}

double datara_rt_f64x2_distance(DataraF64x2 a, DataraF64x2 b) {
    DataraF64x2 diff = datara_rt_f64x2_sub(a, b);
    return sqrt(datara_rt_f64x2_dot(diff, diff));
}

// f64x4 operations
DataraF64x4 datara_rt_f64x4_add(DataraF64x4 a, DataraF64x4 b) {
    DataraF64x4 out;
    out.x = a.x + b.x; out.y = a.y + b.y; out.z = a.z + b.z; out.w = a.w + b.w;
    return out;
}

DataraF64x4 datara_rt_f64x4_sub(DataraF64x4 a, DataraF64x4 b) {
    DataraF64x4 out;
    out.x = a.x - b.x; out.y = a.y - b.y; out.z = a.z - b.z; out.w = a.w - b.w;
    return out;
}

DataraF64x4 datara_rt_f64x4_mul(DataraF64x4 a, DataraF64x4 b) {
    DataraF64x4 out;
    out.x = a.x * b.x; out.y = a.y * b.y; out.z = a.z * b.z; out.w = a.w * b.w;
    return out;
}

DataraF64x4 datara_rt_f64x4_div(DataraF64x4 a, DataraF64x4 b) {
    DataraF64x4 out;
    out.x = a.x / b.x; out.y = a.y / b.y; out.z = a.z / b.z; out.w = a.w / b.w;
    return out;
}

double datara_rt_f64x4_dot(DataraF64x4 a, DataraF64x4 b) {
    return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
}

double datara_rt_f64x4_horizontal_add(DataraF64x4 a) {
    return a.x + a.y + a.z + a.w;
}

DataraF64x4 datara_rt_f64x4_min(DataraF64x4 a, DataraF64x4 b) {
    DataraF64x4 out;
    out.x = a.x < b.x ? a.x : b.x;
    out.y = a.y < b.y ? a.y : b.y;
    out.z = a.z < b.z ? a.z : b.z;
    out.w = a.w < b.w ? a.w : b.w;
    return out;
}

DataraF64x4 datara_rt_f64x4_max(DataraF64x4 a, DataraF64x4 b) {
    DataraF64x4 out;
    out.x = a.x > b.x ? a.x : b.x;
    out.y = a.y > b.y ? a.y : b.y;
    out.z = a.z > b.z ? a.z : b.z;
    out.w = a.w > b.w ? a.w : b.w;
    return out;
}

DataraF64x4 datara_rt_f64x4_lerp(DataraF64x4 a, DataraF64x4 b, double t) {
    DataraF64x4 out;
    out.x = a.x + t * (b.x - a.x);
    out.y = a.y + t * (b.y - a.y);
    out.z = a.z + t * (b.z - a.z);
    out.w = a.w + t * (b.w - a.w);
    return out;
}

DataraF64x4 datara_rt_f64x4_normalize(DataraF64x4 a) {
    double d = datara_rt_f64x4_dot(a, a);
    DataraF64x4 out;
    if (d <= 0.0) {
        out.x = 0.0; out.y = 0.0; out.z = 0.0; out.w = 0.0;
        return out;
    }
    double inv = 1.0 / sqrt(d);
    out.x = a.x * inv;
    out.y = a.y * inv;
    out.z = a.z * inv;
    out.w = a.w * inv;
    return out;
}

double datara_rt_f64x4_distance(DataraF64x4 a, DataraF64x4 b) {
    DataraF64x4 diff = datara_rt_f64x4_sub(a, b);
    return sqrt(datara_rt_f64x4_dot(diff, diff));
}

// ---------------------------------------------------------------------------
// High Performance Vector Algorithms: Array Dot Product & Ray-Sphere
// ---------------------------------------------------------------------------

double datara_rt_dot_f32_array(const float* a, const float* b, int64_t n) {
    int64_t i = 0;
#if defined(DATARA_SIMD_SSE2)
    __m128 sum0 = _mm_setzero_ps();
    __m128 sum1 = _mm_setzero_ps();
    for (; i + 8 <= n; i += 8) {
        __m128 a0 = _mm_loadu_ps(a + i);
        __m128 b0 = _mm_loadu_ps(b + i);
        sum0 = _mm_add_ps(sum0, _mm_mul_ps(a0, b0));
        __m128 a1 = _mm_loadu_ps(a + i + 4);
        __m128 b1 = _mm_loadu_ps(b + i + 4);
        sum1 = _mm_add_ps(sum1, _mm_mul_ps(a1, b1));
    }
    __m128 total = _mm_add_ps(sum0, sum1);
    for (; i + 4 <= n; i += 4) {
        __m128 a0 = _mm_loadu_ps(a + i);
        __m128 b0 = _mm_loadu_ps(b + i);
        total = _mm_add_ps(total, _mm_mul_ps(a0, b0));
    }
    __m128 shuf = _mm_shuffle_ps(total, total, _MM_SHUFFLE(2, 3, 0, 1));
    __m128 sums = _mm_add_ps(total, shuf);
    shuf = _mm_shuffle_ps(sums, sums, _MM_SHUFFLE(1, 0, 3, 2));
    sums = _mm_add_ps(sums, shuf);
    double acc = (double)_mm_cvtss_f32(sums);
#else
    double acc = 0.0;
#endif
    for (; i < n; i++) {
        acc += (double)(a[i] * b[i]);
    }
    return acc;
}

double datara_rt_ray_sphere_intersect_simd(DataraF32x4 ro, DataraF32x4 rd, DataraF32x4 center, double radius) {
    DataraF32x4 oc = datara_rt_f32x4_sub(ro, center);
    double a = datara_rt_f32x4_dot(rd, rd);
    double b = 2.0 * datara_rt_f32x4_dot(oc, rd);
    double c = datara_rt_f32x4_dot(oc, oc) - radius * radius;
    double disc = b * b - 4.0 * a * c;
    if (disc < 0.0) return -1.0;
    return (-b - sqrt(disc)) / (2.0 * a);
}

double datara_rt_ray_sphere_intersect_scalar(double rox, double roy, double roz, double rdx, double rdy, double rdz, double cx, double cy, double cz, double r) {
    double ocx = rox - cx;
    double ocy = roy - cy;
    double ocz = roz - cz;
    double a = rdx * rdx + rdy * rdy + rdz * rdz;
    double b = 2.0 * (ocx * rdx + ocy * rdy + ocz * rdz);
    double c = (ocx * ocx + ocy * ocy + ocz * ocz) - r * r;
    double disc = b * b - 4.0 * a * c;
    if (disc < 0.0) return -1.0;
    return (-b - sqrt(disc)) / (2.0 * a);
}

DataraF32x4 datara_rt_ray_sphere_4x_simd(
    DataraF32x4 ro_x, DataraF32x4 ro_y, DataraF32x4 ro_z,
    DataraF32x4 rd_x, DataraF32x4 rd_y, DataraF32x4 rd_z,
    DataraF32x4 cx,   DataraF32x4 cy,   DataraF32x4 cz,
    DataraF32x4 radius
) {
#if defined(DATARA_SIMD_SSE2)
    __m128 mx_ro = _mm_loadu_ps(&ro_x.x);
    __m128 my_ro = _mm_loadu_ps(&ro_y.x);
    __m128 mz_ro = _mm_loadu_ps(&ro_z.x);

    __m128 mx_rd = _mm_loadu_ps(&rd_x.x);
    __m128 my_rd = _mm_loadu_ps(&rd_y.x);
    __m128 mz_rd = _mm_loadu_ps(&rd_z.x);

    __m128 mx_c = _mm_loadu_ps(&cx.x);
    __m128 my_c = _mm_loadu_ps(&cy.x);
    __m128 mz_c = _mm_loadu_ps(&cz.x);

    __m128 mr = _mm_loadu_ps(&radius.x);

    __m128 oc_x = _mm_sub_ps(mx_ro, mx_c);
    __m128 oc_y = _mm_sub_ps(my_ro, my_c);
    __m128 oc_z = _mm_sub_ps(mz_ro, mz_c);

    __m128 a = _mm_add_ps(_mm_add_ps(_mm_mul_ps(mx_rd, mx_rd), _mm_mul_ps(my_rd, my_rd)), _mm_mul_ps(mz_rd, mz_rd));
    __m128 b_half = _mm_add_ps(_mm_add_ps(_mm_mul_ps(oc_x, mx_rd), _mm_mul_ps(oc_y, my_rd)), _mm_mul_ps(oc_z, mz_rd));
    __m128 b = _mm_mul_ps(_mm_set1_ps(2.0f), b_half);
    __m128 c = _mm_sub_ps(_mm_add_ps(_mm_add_ps(_mm_mul_ps(oc_x, oc_x), _mm_mul_ps(oc_y, oc_y)), _mm_mul_ps(oc_z, oc_z)), _mm_mul_ps(mr, mr));

    __m128 disc = _mm_sub_ps(_mm_mul_ps(b, b), _mm_mul_ps(_mm_set1_ps(4.0f), _mm_mul_ps(a, c)));

    __m128 zero = _mm_setzero_ps();
    __m128 valid_mask = _mm_cmpge_ps(disc, zero);
    __m128 safe_disc = _mm_max_ps(disc, zero);
    __m128 sqrt_disc = _mm_sqrt_ps(safe_disc);

    __m128 neg_b = _mm_sub_ps(zero, b);
    __m128 num = _mm_sub_ps(neg_b, sqrt_disc);
    __m128 den = _mm_mul_ps(_mm_set1_ps(2.0f), a);
    __m128 hit = _mm_div_ps(num, den);

    __m128 neg_one = _mm_set1_ps(-1.0f);
    __m128 res = _mm_or_ps(_mm_and_ps(valid_mask, hit), _mm_andnot_ps(valid_mask, neg_one));

    DataraF32x4 out;
    _mm_storeu_ps(&out.x, res);
    return out;
#else
    DataraF32x4 res;
    float* roxs = &ro_x.x; float* roys = &ro_y.x; float* rozs = &ro_z.x;
    float* rdxs = &rd_x.x; float* rdys = &rd_y.x; float* rdzs = &rd_z.x;
    float* cxs = &cx.x;    float* cys = &cy.x;    float* czs = &cz.x;
    float* rs = &radius.x; float* out = &res.x;
    for (int i = 0; i < 4; i++) {
        float ocx = roxs[i] - cxs[i];
        float ocy = roys[i] - cys[i];
        float ocz = rozs[i] - czs[i];
        float a = rdxs[i]*rdxs[i] + rdys[i]*rdys[i] + rdzs[i]*rdzs[i];
        float b = 2.0f * (ocx*rdxs[i] + ocy*rdys[i] + ocz*rdzs[i]);
        float c = (ocx*ocx + ocy*ocy + ocz*ocz) - rs[i]*rs[i];
        float disc = b*b - 4.0f*a*c;
        if (disc < 0.0f) out[i] = -1.0f;
        else out[i] = (-b - sqrtf(disc)) / (2.0f * a);
    }
    return res;
#endif
}

DataraF32x4 datara_rt_ray_sphere_4x_scalar(
    DataraF32x4 ro_x, DataraF32x4 ro_y, DataraF32x4 ro_z,
    DataraF32x4 rd_x, DataraF32x4 rd_y, DataraF32x4 rd_z,
    DataraF32x4 cx,   DataraF32x4 cy,   DataraF32x4 cz,
    DataraF32x4 radius
) {
    DataraF32x4 res;
    float* roxs = &ro_x.x; float* roys = &ro_y.x; float* rozs = &ro_z.x;
    float* rdxs = &rd_x.x; float* rdys = &rd_y.x; float* rdzs = &rd_z.x;
    float* cxs = &cx.x;    float* cys = &cy.x;    float* czs = &cz.x;
    float* rs = &radius.x; float* out = &res.x;
    for (int i = 0; i < 4; i++) {
        float ocx = roxs[i] - cxs[i];
        float ocy = roys[i] - cys[i];
        float ocz = rozs[i] - czs[i];
        float a = rdxs[i]*rdxs[i] + rdys[i]*rdys[i] + rdzs[i]*rdzs[i];
        float b = 2.0f * (ocx*rdxs[i] + ocy*rdys[i] + ocz*rdzs[i]);
        float c = (ocx*ocx + ocy*ocy + ocz*ocz) - rs[i]*rs[i];
        float disc = b*b - 4.0f*a*c;
        if (disc < 0.0f) {
            out[i] = -1.0f;
        } else {
            out[i] = (-b - sqrtf(disc)) / (2.0f * a);
        }
    }
    return res;
}

void datara_rt_ray_sphere_batch_simd(
    const float* rox, const float* roy, const float* roz,
    const float* rdx, const float* rdy, const float* rdz,
    float cx, float cy, float cz, float r,
    float* out_t, int64_t count
) {
    int64_t i = 0;
#if defined(DATARA_SIMD_SSE2)
    __m128 vcx = _mm_set1_ps(cx);
    __m128 vcy = _mm_set1_ps(cy);
    __m128 vcz = _mm_set1_ps(cz);
    __m128 vr = _mm_set1_ps(r);
    __m128 vr2 = _mm_mul_ps(vr, vr);
    __m128 zero = _mm_setzero_ps();
    __m128 two = _mm_set1_ps(2.0f);
    __m128 four = _mm_set1_ps(4.0f);
    __m128 neg_one = _mm_set1_ps(-1.0f);

    for (; i + 4 <= count; i += 4) {
        __m128 mx_ro = _mm_loadu_ps(rox + i);
        __m128 my_ro = _mm_loadu_ps(roy + i);
        __m128 mz_ro = _mm_loadu_ps(roz + i);

        __m128 mx_rd = _mm_loadu_ps(rdx + i);
        __m128 my_rd = _mm_loadu_ps(rdy + i);
        __m128 mz_rd = _mm_loadu_ps(rdz + i);

        __m128 oc_x = _mm_sub_ps(mx_ro, vcx);
        __m128 oc_y = _mm_sub_ps(my_ro, vcy);
        __m128 oc_z = _mm_sub_ps(mz_ro, vcz);

        __m128 a = _mm_add_ps(_mm_add_ps(_mm_mul_ps(mx_rd, mx_rd), _mm_mul_ps(my_rd, my_rd)), _mm_mul_ps(mz_rd, mz_rd));
        __m128 b_half = _mm_add_ps(_mm_add_ps(_mm_mul_ps(oc_x, mx_rd), _mm_mul_ps(oc_y, my_rd)), _mm_mul_ps(oc_z, mz_rd));
        __m128 b = _mm_mul_ps(two, b_half);
        __m128 c = _mm_sub_ps(_mm_add_ps(_mm_add_ps(_mm_mul_ps(oc_x, oc_x), _mm_mul_ps(oc_y, oc_y)), _mm_mul_ps(oc_z, oc_z)), vr2);

        __m128 disc = _mm_sub_ps(_mm_mul_ps(b, b), _mm_mul_ps(four, _mm_mul_ps(a, c)));
        __m128 valid_mask = _mm_cmpge_ps(disc, zero);
        __m128 safe_disc = _mm_max_ps(disc, zero);
        __m128 sqrt_disc = _mm_sqrt_ps(safe_disc);

        __m128 neg_b = _mm_sub_ps(zero, b);
        __m128 num = _mm_sub_ps(neg_b, sqrt_disc);
        __m128 den = _mm_mul_ps(two, a);
        __m128 hit = _mm_div_ps(num, den);

        __m128 res = _mm_or_ps(_mm_and_ps(valid_mask, hit), _mm_andnot_ps(valid_mask, neg_one));
        _mm_storeu_ps(out_t + i, res);
    }
#endif
    for (; i < count; i++) {
        float ocx = rox[i] - cx;
        float ocy = roy[i] - cy;
        float ocz = roz[i] - cz;
        float a = rdx[i]*rdx[i] + rdy[i]*rdy[i] + rdz[i]*rdz[i];
        float b = 2.0f * (ocx*rdx[i] + ocy*rdy[i] + ocz*rdz[i]);
        float c = (ocx*ocx + ocy*ocy + ocz*ocz) - r*r;
        float disc = b*b - 4.0f*a*c;
        if (disc < 0.0f) out_t[i] = -1.0f;
        else out_t[i] = (-b - sqrtf(disc)) / (2.0f * a);
    }
}

#if defined(_MSC_VER)
#pragma optimize("", off)
#endif
void datara_rt_ray_sphere_batch_scalar(
    const float* rox, const float* roy, const float* roz,
    const float* rdx, const float* rdy, const float* rdz,
    float cx, float cy, float cz, float r,
    float* out_t, int64_t count
) {
    for (int64_t i = 0; i < count; i++) {
        float ocx = rox[i] - cx;
        float ocy = roy[i] - cy;
        float ocz = roz[i] - cz;
        float a = rdx[i]*rdx[i] + rdy[i]*rdy[i] + rdz[i]*rdz[i];
        float b = 2.0f * (ocx*rdx[i] + ocy*rdy[i] + ocz*rdz[i]);
        float c = (ocx*ocx + ocy*ocy + ocz*ocz) - r*r;
        float disc = b*b - 4.0f*a*c;
        if (disc < 0.0f) {
            out_t[i] = -1.0f;
        } else {
            out_t[i] = (-b - sqrtf(disc)) / (2.0f * a);
        }
    }
}
#if defined(_MSC_VER)
#pragma optimize("", on)
#endif

double datara_rt_fma(double a, double b, double c) {
    return fma(a, b, c);
}

float datara_rt_fmaf(float a, float b, float c) {
    return fmaf(a, b, c);
}



