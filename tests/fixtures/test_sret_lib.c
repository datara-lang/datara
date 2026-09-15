#include "test_sret_lib.h"

Vec2d make_vec2d(double x, double y) {
    Vec2d v;
    v.x = x;
    v.y = y;
    return v;
}

Vec2d add_vec2d(Vec2d a, Vec2d b) {
    Vec2d r;
    r.x = a.x + b.x;
    r.y = a.y + b.y;
    return r;
}

double vec2d_dot(Vec2d a, Vec2d b) {
    return a.x * b.x + a.y * b.y;
}

Pair make_pair(long long a, long long b) {
    Pair p;
    p.a = a;
    p.b = b;
    return p;
}

long long sum_pair(Pair p) {
    return p.a + p.b;
}

Word make_word(long long v) {
    Word w;
    w.v = v;
    return w;
}

long long word_value(Word w) {
    return w.v;
}

Triple make_triple(long long a, long long b, long long c) {
    Triple t;
    t.a = a;
    t.b = b;
    t.c = c;
    return t;
}

Quad make_quad(long long w, long long x, long long y, long long z) {
    Quad q;
    q.w = w;
    q.x = x;
    q.y = y;
    q.z = z;
    return q;
}

FloatTriple make_float_triple(double a, double b, double c) {
    FloatTriple f;
    f.a = a;
    f.b = b;
    f.c = c;
    return f;
}

Mixed make_mixed(int a, int b, long long c) {
    Mixed m;
    m.a = a;
    m.b = b;
    m.c = c;
    return m;
}
