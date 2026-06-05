// spectralnorm (benchmarksgame): largest eigenvalue of A[i][j] =
// 1/((i+j)(i+j+1)/2 + i + 1), via 10 power-method iterations of AᵀA
// on the all-ones vector. Serial port. Reads n from argv (default 100).

package main

import "core:fmt"
import "core:math"
import "core:os"
import "core:strconv"

a :: proc(i, j: int) -> f64 {
  return 1.0 / f64((i + j) * (i + j + 1) / 2 + i + 1)
}

mult_av :: proc(v, out: []f64) {
  for i in 0 ..< len(out) {
    sum := 0.0
    for j in 0 ..< len(v) {
      sum += a(i, j) * v[j]
    }
    out[i] = sum
  }
}

mult_atv :: proc(v, out: []f64) {
  for i in 0 ..< len(out) {
    sum := 0.0
    for j in 0 ..< len(v) {
      sum += a(j, i) * v[j]
    }
    out[i] = sum
  }
}

mult_atav :: proc(v, out, tmp: []f64) {
  mult_av(v, tmp)
  mult_atv(tmp, out)
}

main :: proc() {
  n := 100
  if len(os.args) > 1 {
    if value, ok := strconv.parse_int(os.args[1]); ok {
      n = value
    }
  }

  u := make([]f64, n)
  v := make([]f64, n)
  tmp := make([]f64, n)
  defer delete(u)
  defer delete(v)
  defer delete(tmp)

  for i in 0 ..< n {
    u[i] = 1.0
  }

  for _ in 0 ..< 10 {
    mult_atav(u, v, tmp)
    mult_atav(v, u, tmp)
  }

  v_bv := 0.0
  vv := 0.0
  for i in 0 ..< n {
    v_bv += u[i] * v[i]
    vv += v[i] * v[i]
  }

  fmt.printf("%.9f\n", math.sqrt(v_bv / vv))
}
