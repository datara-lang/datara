#include <stdio.h>
#include <stdlib.h>
int main() {
    int n = 256;
    long long *a = malloc(n*n*sizeof(long long));
    long long *b = malloc(n*n*sizeof(long long));
    for (long long i = 0; i < n*n; i++) { a[i] = i % 7; b[i] = (i*3) % 5; }
    long long sum = 0;
    for (int r = 0; r < n; r++)
        for (int c = 0; c < n; c++) {
            long long acc = 0;
            for (int k = 0; k < n; k++) acc += a[r*n+k] * b[k*n+c];
            sum += acc % 13;
        }
    printf("%lld\n", sum);
    return 0;
}
