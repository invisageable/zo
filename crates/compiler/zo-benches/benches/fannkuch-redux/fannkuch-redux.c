// fannkuch-redux (benchmarksgame): for every permutation of n
// elements, flip the prefix whose length is the first element until
// that element is 0, counting flips. Reports the maximum flip count
// and a sign-alternating checksum.
//
// Serial port — the harness builds with plain clang (no -fopenmp),
// so the omitted omp pragma's parallel-for runs as an ordinary loop.
// Reads n from argv[1].

#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>

int main(int argc, char **argv) {
  const int n = argc > 1 ? atoi(argv[1]) : 7;

  long perm_max = 1;
  for (int i = 1; i <= n; ++i)
    perm_max *= i;

  int current[16], temp[16], count[16];
  for (int i = 0; i < n; ++i) {
    current[i] = i;
    count[i] = 0;
  }

  long checksum = 0;
  int max_flips = 0;

  for (long perm_index = 0;; ++perm_index) {
    if (current[0] > 0) {
      for (int i = 0; i < n; ++i)
        temp[i] = current[i];

      int flip_count = 1;
      int first_value = current[0];

      while (temp[first_value] != 0) {
        const int new_first_value = temp[first_value];
        temp[first_value] = first_value;

        if (first_value > 2) {
          int lo = 1, hi = first_value - 1;
          while (lo < hi) {
            const int swap = temp[lo];
            temp[lo] = temp[hi];
            temp[hi] = swap;
            ++lo;
            --hi;
          }
        }

        first_value = new_first_value;
        ++flip_count;
      }

      checksum += (perm_index % 2 == 0) ? flip_count : -flip_count;
      if (flip_count > max_flips)
        max_flips = flip_count;
    }

    if (perm_index >= perm_max - 1)
      break;

    // next permutation — a factorial-radix increment of `count`.
    int first_value = current[1];
    current[1] = current[0];
    current[0] = first_value;

    int i = 1;
    while (count[i] >= i) {
      count[i] = 0;
      ++i;

      const int new_first_value = current[1];
      current[0] = new_first_value;

      for (int j = 1; j < i; ++j)
        current[j] = current[j + 1];

      current[i] = first_value;
      first_value = new_first_value;
    }

    ++count[i];
  }

  printf("%ld\nPfannkuchen(%d) = %d\n", checksum, n, max_flips);

  return 0;
}
