// runtime kernel: register pressure (C reference).
#include <stdio.h>

long mix(long n) {
  long a = 1, b = 2, c = 3, d = 4, e = 5, f = 6, g = 7, h = 8, i = 0;
  while (i < n) {
    a = a + h; b = b + a; c = c + b; d = d + c;
    e = e + d; f = f + e; g = g + f; h = h + g;
    i = i + 1;
  }
  return a + b + c + d + e + f + g + h;
}

int main(void) {
  printf("%ld\n", mix(50000000));
  return 0;
}
