#include <stdio.h>
int main(void) {
    long long acc = 0;
    for (long long i = 0; i < 100000000; i++) acc += i & 255;
    printf("%lld\n", acc);
    return 0;
}
