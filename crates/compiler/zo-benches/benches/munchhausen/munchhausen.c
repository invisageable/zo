#include <stdio.h>

#define MAX 440000000

int pow_int(int b, int e) {
  int result = 1;
  for (int i = 0; i < e; i++) result *= b;
  return result;
}

int is_munchhausen(int n, const int *pwr) {
  int sum = 0;
  int temp = n;
  while (temp > 0) {
    sum += pwr[temp % 10];
    temp /= 10;
  }
  return sum == n;
}

int main(void) {
  int pwr[10];
  for (int i = 0; i < 10; i++) pwr[i] = pow_int(i, i);
  for (int n = 1; n <= MAX; n++) {
    if (is_munchhausen(n, pwr)) printf("%d\n", n);
  }
  return 0;
}
