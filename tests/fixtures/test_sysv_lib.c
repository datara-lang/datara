#include "test_sysv_lib.h"

I64Pair make_i64_pair(long long a, long long b) {
    I64Pair p;
    p.a = a;
    p.b = b;
    return p;
}

F64Pair make_f64_pair(double x, double y) {
    F64Pair p;
    p.x = x;
    p.y = y;
    return p;
}

MixedIF make_mixed_if(long long a, double x) {
    MixedIF m;
    m.a = a;
    m.x = x;
    return m;
}

MixedFI make_mixed_fi(double x, long long a) {
    MixedFI m;
    m.x = x;
    m.a = a;
    return m;
}

Word make_word(long long v) {
    Word w;
    w.v = v;
    return w;
}

long long word_value(Word w) {
    return w.v;
}

/* Unused ellipsis: no va_list is consumed, the function only exists so the
   import gate sees a variadic struct-return declaration. */
I64Pair make_i64_pair_variadic(long long a, ...) {
    I64Pair p;
    p.a = a;
    p.b = 0;
    return p;
}

Wide3 make_wide3(int a, int b, long long c) {
    Wide3 w;
    w.a = a;
    w.b = b;
    w.c = c;
    return w;
}

Triple make_triple(long long a, long long b, long long c) {
    Triple t;
    t.a = a;
    t.b = b;
    t.c = c;
    return t;
}
