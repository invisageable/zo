// fannkuch-redux (benchmarksgame): for every permutation of n
// elements, flip the prefix whose length is the first element until
// that element is 0, counting flips. Reports the maximum flip count
// and a sign-alternating checksum. Serial port. Reads n from argv
// (default 7).

package main

import "core:fmt"
import "core:os"
import "core:strconv"

fannkuch :: proc(n: int) -> (int, int) {
  current := make([]int, n + 1)
  temp := make([]int, n + 1)
  count := make([]int, n + 1)
  defer delete(current)
  defer delete(temp)
  defer delete(count)

  for i in 0 ..< n {
    current[i] = i
  }

  perm_max := 1
  for i in 1 ..= n {
    perm_max *= i
  }

  max_flips := 0
  checksum := 0
  perm_index := 0

  for {
    if current[0] > 0 {
      for i in 0 ..< n {
        temp[i] = current[i]
      }

      flip_count := 1
      first_value := current[0]

      for temp[first_value] != 0 {
        new_first := temp[first_value]
        temp[first_value] = first_value

        if first_value > 2 {
          lo := 1
          hi := first_value - 1
          for lo < hi {
            temp[lo], temp[hi] = temp[hi], temp[lo]
            lo += 1
            hi -= 1
          }
        }

        first_value = new_first
        flip_count += 1
      }

      if perm_index % 2 == 0 {
        checksum += flip_count
      } else {
        checksum -= flip_count
      }

      if flip_count > max_flips {
        max_flips = flip_count
      }
    }

    if perm_index >= perm_max - 1 {
      break
    }

    perm_index += 1

    // next permutation — a factorial-radix increment of `count`.
    first_value := current[1]
    current[1] = current[0]
    current[0] = first_value

    i := 1
    for count[i] >= i {
      count[i] = 0
      i += 1

      new_first := current[1]
      current[0] = new_first

      for j in 1 ..< i {
        current[j] = current[j + 1]
      }

      current[i] = first_value
      first_value = new_first
    }

    count[i] += 1
  }

  return checksum, max_flips
}

main :: proc() {
  n := 7
  if len(os.args) > 1 {
    if value, ok := strconv.parse_int(os.args[1]); ok {
      n = value
    }
  }

  checksum, max_flips := fannkuch(n)

  fmt.printf("%d\nPfannkuchen(%d) = %d\n", checksum, n, max_flips)
}
