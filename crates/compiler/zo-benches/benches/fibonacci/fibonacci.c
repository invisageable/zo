#include <assert.h>
#include <stdio.h>

int fib(int n) {
  if (n == 0) {
    return 0;
  } else {
    if (n <= 2) {
      return 1;
    } else {
      return fib(n - 1) + fib(n - 2);
    }
  }
}

int main() {
  assert(fib(8) == 21);
  assert(fib(15) == 610);
  printf("%d\n", fib(8));
  printf("%d\n", fib(15));

  return 0;
}
