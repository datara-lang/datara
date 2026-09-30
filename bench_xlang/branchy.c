#include <stdio.h>
static long heavy(long acc, long i) {
    long h = acc;
    for (long k = 0; k < 16; k++) h += (i ^ k) & 63;
    return h;
}
int main(void) {
    long acc = 0;
    for (long i = 0; i < 50000000; i++) {
        if ((i & 1023) == 0) acc = heavy(acc, i);
        else acc += 1;
    }
    printf("%ld\n", acc);
    return 0;
}
