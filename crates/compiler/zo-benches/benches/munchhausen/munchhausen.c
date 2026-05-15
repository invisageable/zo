#include <stdio.h>

#define MAX 440000000L

long pow_int(long b, long e) {
  long result = 1;
  for (long i = 0; i < e; i++) result *= b;
  return result;
}

long is_munchhausen(long n, const long *pwr) {
  long sum = 0;
  long temp = n;
  while (temp > 0) {
    sum += pwr[temp % 10];
    temp /= 10;
  }
  return sum == n;
}

int main(void) {
  long pwr[10];
  for (long i = 0; i < 10; i++) pwr[i] = pow_int(i, i);
  for (long n = 1; n <= MAX; n++) {
    if (is_munchhausen(n, pwr)) printf("%ld\n", n);
  }
  return 0;
}
