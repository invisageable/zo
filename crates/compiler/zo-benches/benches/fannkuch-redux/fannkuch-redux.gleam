// fannkuch-redux (benchmarksgame): for every permutation of n
// elements, flip the prefix whose length is the first element until
// that element is 0, counting flips. Reports the maximum flip count
// and a sign-alternating checksum.
//
// Gleam is immutable and loop-free, so the three working arrays are
// `Dict(Int, Int)` threaded through tail-recursive functions (BEAM
// does tail-call elimination, so the deep recursion runs in constant
// stack). The harness builds a Gleam *project* and can't pass argv,
// so n is fixed at 7 — matching c/go/rust/odin/zo at n=7: 228 / 16.

import gleam/dict.{type Dict}
import gleam/int
import gleam/io

type Arr =
  Dict(Int, Int)

const n = 7

fn get(array: Arr, index: Int) -> Int {
  case dict.get(array, index) {
    Ok(value) -> value
    Error(_) -> 0
  }
}

fn identity(index: Int, acc: Arr) -> Arr {
  case index < n {
    False -> acc
    True -> identity(index + 1, dict.insert(acc, index, index))
  }
}

fn factorial(index: Int, acc: Int) -> Int {
  case index > n {
    True -> acc
    False -> factorial(index + 1, acc * index)
  }
}

// Reverse the inclusive range [lo, hi] of `array` by swapping ends.
fn reverse_range(array: Arr, lo: Int, hi: Int) -> Arr {
  case lo < hi {
    False -> array
    True -> {
      let low = get(array, lo)
      let high = get(array, hi)

      reverse_range(
        dict.insert(dict.insert(array, lo, high), hi, low),
        lo + 1,
        hi - 1,
      )
    }
  }
}

// Count the flips for one permutation. `temp` starts as a copy of the
// current permutation (dicts are immutable, so passing it is a copy).
fn flip(temp: Arr, first_value: Int, flips: Int) -> Int {
  case get(temp, first_value) != 0 {
    False -> flips
    True -> {
      let new_first = get(temp, first_value)
      let flipped = dict.insert(temp, first_value, first_value)
      let rotated = case first_value > 2 {
        True -> reverse_range(flipped, 1, first_value - 1)
        False -> flipped
      }

      flip(rotated, new_first, flips + 1)
    }
  }
}

// Shift `current[lo..=hi]` left by one (`current[j] = current[j+1]`).
fn shift_left(current: Arr, j: Int, hi: Int) -> Arr {
  case j < hi {
    False -> current
    True -> shift_left(dict.insert(current, j, get(current, j + 1)), j + 1, hi)
  }
}

// The factorial-radix carry that advances `count` and rebuilds the
// permutation digit by digit.
fn carry(
  current: Arr,
  count: Arr,
  index: Int,
  first_value: Int,
) -> #(Arr, Arr) {
  case get(count, index) >= index {
    False -> #(current, dict.insert(count, index, get(count, index) + 1))
    True -> {
      let next_index = index + 1
      let new_first = get(current, 1)
      let shifted =
        shift_left(dict.insert(current, 0, new_first), 1, next_index)

      carry(
        dict.insert(shifted, next_index, first_value),
        dict.insert(count, index, 0),
        next_index,
        new_first,
      )
    }
  }
}

fn next_permutation(current: Arr, count: Arr) -> #(Arr, Arr) {
  let first_value = get(current, 1)
  let swapped = dict.insert(dict.insert(current, 1, get(current, 0)), 0, first_value)

  carry(swapped, count, 1, first_value)
}

fn run(
  current: Arr,
  count: Arr,
  perm_index: Int,
  perm_max: Int,
  checksum: Int,
  max_flips: Int,
) -> #(Int, Int) {
  let #(checksum, max_flips) = case get(current, 0) > 0 {
    False -> #(checksum, max_flips)
    True -> {
      let flips = flip(current, get(current, 0), 1)
      let checksum = case int.is_even(perm_index) {
        True -> checksum + flips
        False -> checksum - flips
      }
      let max_flips = case flips > max_flips {
        True -> flips
        False -> max_flips
      }

      #(checksum, max_flips)
    }
  }

  case perm_index >= perm_max - 1 {
    True -> #(checksum, max_flips)
    False -> {
      let #(current, count) = next_permutation(current, count)

      run(current, count, perm_index + 1, perm_max, checksum, max_flips)
    }
  }
}

pub fn main() {
  let perm_max = factorial(1, 1)
  let #(checksum, max_flips) =
    run(identity(0, dict.new()), dict.new(), 0, perm_max, 0, 0)

  io.println(int.to_string(checksum))
  io.println(
    "Pfannkuchen(" <> int.to_string(n) <> ") = " <> int.to_string(max_flips),
  )
}
