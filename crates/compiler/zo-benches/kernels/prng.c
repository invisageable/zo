// runtime kernel: latency-bound arithmetic (C reference).
#include <stdio.h>

long prng(long iters) {
  long x = 1, i = 0;
  while (i < iters) {
    x = x * 1103515245 + 12345;
    i = i + 1;
  }
  return x;
}

int main(void) {
  printf("%ld\n", prng(200000000));
  return 0;
}
