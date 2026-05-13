package main

import "core:fmt"

fib :: proc(n: i64) -> i64 {
  if n == 0 {
    return 0
  } else {
    if n <= 2 {
      return 1
    } else {
      return fib(n - 1) + fib(n - 2)
    }
  }
}

main :: proc() {
  assert(fib(8) == 21)
  assert(fib(15) == 610)
  fmt.println(fib(8))
  fmt.println(fib(15))
}
