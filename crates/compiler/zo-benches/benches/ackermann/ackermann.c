#include <assert.h>

int ack(int m, int n) {
  if (m == 0) {
    return n + 1;
  } else {
    if (n == 0) {
      return ack(m - 1, 1);
    } else {
      return ack(m - 1, ack(m, n - 1));
    }
  }
}

int main() {
  assert(ack(0, 0) == 1);
  assert(ack(3, 2) == 29);
  assert(ack(3, 4) == 125);

  return 0;
}