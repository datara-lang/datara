import sys
sys.setrecursionlimit(10000)
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
total = 0
for i in range(10):
    total += fib(30)
