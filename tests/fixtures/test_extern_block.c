/* C implementation of the extern "C" block fixture (v1.3.2 M2).
   Compiled into a static library by the integration test and linked
   against the Datara executable. */

long long block_add(long long a, long long b) {
    return a + b;
}

long long block_scale(long long x, long long k) {
    return x * k;
}

long long block_negate(long long v) {
    return -v;
}

long long block_plain(long long v) {
    return v * 10;
}
