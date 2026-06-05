// spectralnorm (benchmarksgame): largest eigenvalue of the infinite
// matrix A, A[i][j] = 1/((i+j)(i+j+1)/2 + i + 1), via 10 power-method
// iterations of AᵀA on the all-ones vector. Prints sqrt((u·v)/(v·v)).
//
// Gleam is immutable and loop-free, so each vector is a Dict(Int,
// Float) rebuilt by tail-recursive functions (BEAM tail-call
// elimination keeps the deep recursion in constant stack). The
// harness builds a project and can't pass argv, so n is fixed at 100
// — the reference size; the value matches c/go/rust/odin/zo.

import gleam/dict.{type Dict}
import gleam/float
import gleam/int
import gleam/io

type Vec =
  Dict(Int, Float)

const n = 100

// A[i][j] = 1 / ((i+j)(i+j+1)/2 + i + 1).
fn a(i: Int, j: Int) -> Float {
  let d = { i + j } * { i + j + 1 } / 2 + i + 1

  1.0 /. int.to_float(d)
}

fn get(v: Vec, i: Int) -> Float {
  case dict.get(v, i) {
    Ok(value) -> value
    Error(_) -> 0.0
  }
}

// Σ over j of coeff(i,j) * v[j], where coeff is A (or Aᵀ when
// `transpose`).
fn row_sum(transpose: Bool, i: Int, j: Int, v: Vec, acc: Float) -> Float {
  case j < n {
    False -> acc
    True -> {
      let coeff = case transpose {
        True -> a(j, i)
        False -> a(i, j)
      }

      row_sum(transpose, i, j + 1, v, acc +. coeff *. get(v, j))
    }
  }
}

fn mult(transpose: Bool, i: Int, v: Vec, out: Vec) -> Vec {
  case i < n {
    False -> out
    True ->
      mult(
        transpose,
        i + 1,
        v,
        dict.insert(out, i, row_sum(transpose, i, 0, v, 0.0)),
      )
  }
}

// out = AᵀA · v, via out = Aᵀ · (A · v).
fn mult_atav(v: Vec) -> Vec {
  mult(True, 0, mult(False, 0, v, dict.new()), dict.new())
}

fn iterate(step: Int, u: Vec, v: Vec) -> #(Vec, Vec) {
  case step < 10 {
    False -> #(u, v)
    True -> {
      let next_v = mult_atav(u)

      iterate(step + 1, mult_atav(next_v), next_v)
    }
  }
}

fn dots(i: Int, u: Vec, v: Vec, v_bv: Float, vv: Float) -> #(Float, Float) {
  case i < n {
    False -> #(v_bv, vv)
    True ->
      dots(
        i + 1,
        u,
        v,
        v_bv +. get(u, i) *. get(v, i),
        vv +. get(v, i) *. get(v, i),
      )
  }
}

fn ones(i: Int, acc: Vec) -> Vec {
  case i < n {
    False -> acc
    True -> ones(i + 1, dict.insert(acc, i, 1.0))
  }
}

pub fn main() {
  let #(u, v) = iterate(0, ones(0, dict.new()), dict.new())
  let #(v_bv, vv) = dots(0, u, v, 0.0, 0.0)

  let result = case float.square_root(v_bv /. vv) {
    Ok(value) -> value
    Error(_) -> 0.0
  }

  io.println(float.to_string(result))
}
