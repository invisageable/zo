// runtime kernel: call + frame overhead (C reference).
#include <stdio.h>

long fib(long n) {
  if (n < 2) {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

int main(void) {
  printf("%ld\n", fib(38));
  return 0;
}
