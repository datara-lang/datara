#include <stdio.h>
long long fib(long long n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}
int main(void) {
    long long total = 0;
    for (long long i = 0; i < 10; i++) total += fib(30);
    printf("%lld\n", total);
    return 0;
}
