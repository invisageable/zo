package main

import "core:fmt"

WIDTH :: 800
HEIGHT :: 600
MAX_ITER :: 100

main :: proc() {
  width_f := f64(WIDTH)
  height_f := f64(HEIGHT)

  checksum: i64 = 0

  for y in 0..<HEIGHT {
    for x in 0..<WIDTH {
      cr := f64(x) / width_f * 3.0 - 2.0
      ci := f64(y) / height_f * 2.24 - 1.12

      zr := 0.0
      zi := 0.0
      iter: i64 = 0

      for zr * zr + zi * zi <= 4.0 && iter < MAX_ITER {
        temp := zr * zr - zi * zi + cr
        zi = 2.0 * zr * zi + ci
        zr = temp
        iter += 1
      }

      checksum += iter
    }
  }

  fmt.println(checksum)
}
