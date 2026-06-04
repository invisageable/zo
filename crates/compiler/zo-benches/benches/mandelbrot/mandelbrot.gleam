// Mandelbrot escape-time over an 800x600 grid — sequential compute
// benchmark. Gleam is immutable and loop-free, so the nested loops
// become tail-recursive functions (BEAM does tail-call elimination,
// so the deep recursion runs in constant stack). Sums every pixel's
// escape count into a checksum; matches c/go/rust/odin/zo: 13167508.

import gleam/int
import gleam/io

const width = 800

const height = 600

const max_iter = 100

fn escape(zr: Float, zi: Float, cr: Float, ci: Float, iter: Int) -> Int {
  case zr *. zr +. zi *. zi <=. 4.0 && iter < max_iter {
    False -> iter
    True ->
      escape(
        zr *. zr -. zi *. zi +. cr,
        2.0 *. zr *. zi +. ci,
        cr,
        ci,
        iter + 1,
      )
  }
}

fn row(x: Int, y: Int, width_f: Float, height_f: Float, acc: Int) -> Int {
  case x < width {
    False -> acc
    True -> {
      let cr = int.to_float(x) /. width_f *. 3.0 -. 2.0
      let ci = int.to_float(y) /. height_f *. 2.24 -. 1.12

      row(x + 1, y, width_f, height_f, acc + escape(0.0, 0.0, cr, ci, 0))
    }
  }
}

fn rows(y: Int, width_f: Float, height_f: Float, acc: Int) -> Int {
  case y < height {
    False -> acc
    True -> rows(y + 1, width_f, height_f, row(0, y, width_f, height_f, acc))
  }
}

pub fn main() {
  let width_f = int.to_float(width)
  let height_f = int.to_float(height)

  io.println(int.to_string(rows(0, width_f, height_f, 0)))
}
