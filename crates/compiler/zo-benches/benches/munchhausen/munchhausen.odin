package main

import "core:fmt"

MAX :: 440_000_000

pow_int :: proc(b: int, e: int) -> int {
  result := 1
  for i := 0; i < e; i += 1 {
    result *= b
  }
  return result
}

is_munchhausen :: proc(n: int, pwr: []int) -> bool {
  sum := 0
  temp := n
  for temp > 0 {
    sum += pwr[temp % 10]
    temp /= 10
  }
  return sum == n
}

main :: proc() {
  pwr: [10]int
  for i := 0; i < 10; i += 1 {
    pwr[i] = pow_int(i, i)
  }
  for n := 1; n <= MAX; n += 1 {
    if is_munchhausen(n, pwr[:]) {
      fmt.println(n)
    }
  }
}
