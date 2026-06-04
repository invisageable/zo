// Münchhausen numbers up to 440M — a number equal to the sum of
// its digits raised to themselves. Gleam / BEAM: loop-free, so the
// scan and the per-number digit walk are tail-recursive. `pow`
// hardcodes d^d for digits 0..9 (0^0 = 1, matching zo's pow_i) to
// avoid O(n) list indexing on the hot path.

import gleam/int
import gleam/io

const max = 440_000_000

fn pow(digit: Int) -> Int {
  case digit {
    0 -> 1
    1 -> 1
    2 -> 4
    3 -> 27
    4 -> 256
    5 -> 3125
    6 -> 46_656
    7 -> 823_543
    8 -> 16_777_216
    9 -> 387_420_489
    _ -> 0
  }
}

fn digit_sum(temp: Int, sum: Int) -> Int {
  case temp > 0 {
    True -> digit_sum(temp / 10, sum + pow(temp % 10))
    False -> sum
  }
}

fn is_munchhausen(n: Int) -> Bool {
  digit_sum(n, 0) == n
}

fn search(n: Int) -> Nil {
  case n > max {
    True -> Nil
    False -> {
      case is_munchhausen(n) {
        True -> io.println(int.to_string(n))
        False -> Nil
      }

      search(n + 1)
    }
  }
}

pub fn main() {
  search(1)
}
