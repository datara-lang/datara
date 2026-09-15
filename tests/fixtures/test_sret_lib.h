#ifndef TEST_SRET_LIB_H
#define TEST_SRET_LIB_H

/* 16 bytes, two 8-byte floats: the classic hidden-sret shape on
   Microsoft x64 (any struct larger than 8 bytes is returned through a
   caller-allocated buffer). Accepted (<= MAX_SRET_BYTES). */
typedef struct {
    double x;
    double y;
} Vec2d;

/* 16 bytes, two 8-byte integers: the integer counterpart of Vec2d.
   Accepted (<= MAX_SRET_BYTES). */
typedef struct {
    long long a;
    long long b;
} Pair;

/* 8 bytes: returned in RAX, the pre-existing register path. */
typedef struct {
    long long v;
} Word;

/* 24 bytes: three 8-byte scalars, layout-compatible but larger than the
   v1.3.2 sret window. Must keep the E0962 compile-time rejection. */
typedef struct {
    long long a;
    long long b;
    long long c;
} Triple;

/* 32 bytes: four 8-byte scalars, also above the sret window. Rejected. */
typedef struct {
    long long w;
    long long x;
    long long y;
    long long z;
} Quad;

/* 24 bytes of floats, above the sret window: the float-class rejection
   case. Rejected. */
typedef struct {
    double a;
    double b;
    double c;
} FloatTriple;

/* 16 bytes whose C layout Datara cannot read back: fields land at
   offsets 0/4/8 while the Datara object layout puts fields at 0/8/16.
   Must keep the E0962 compile-time rejection. */
typedef struct {
    int a;
    int b;
    long long c;
} Mixed;

Vec2d make_vec2d(double x, double y);
Vec2d add_vec2d(Vec2d a, Vec2d b);
double vec2d_dot(Vec2d a, Vec2d b);

Pair make_pair(long long a, long long b);
long long sum_pair(Pair p);

Word make_word(long long v);
long long word_value(Word w);

Triple make_triple(long long a, long long b, long long c);
Quad make_quad(long long w, long long x, long long y, long long z);
FloatTriple make_float_triple(double a, double b, double c);
Mixed make_mixed(int a, int b, long long c);

#endif
