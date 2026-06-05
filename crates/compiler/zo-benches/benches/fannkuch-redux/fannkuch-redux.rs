// fannkuch-redux (benchmarksgame): for every permutation of n
// elements, flip the prefix whose length is the first element until
// that element is 0, counting flips. Reports the maximum flip count
// and a sign-alternating checksum. Serial port (the harness builds
// with plain rustc, so no rayon). Reads n from argv (default 7).

use std::env;

fn fannkuch(n: usize) -> (i64, i32) {
  let mut current = vec![0i32; n + 1];
  let mut temp = vec![0i32; n + 1];
  let mut count = vec![0i32; n + 1];

  for i in 0..n {
    current[i] = i as i32;
  }

  let mut perm_max: i64 = 1;
  for i in 1..=n {
    perm_max *= i as i64;
  }

  let mut max_flips = 0;
  let mut checksum: i64 = 0;
  let mut perm_index: i64 = 0;

  loop {
    if current[0] > 0 {
      temp.copy_from_slice(&current);

      let mut flip_count = 1;
      let mut first_value = current[0] as usize;

      while temp[first_value] != 0 {
        let new_first = temp[first_value];
        temp[first_value] = first_value as i32;

        if first_value > 2 {
          temp[1..first_value].reverse();
        }

        first_value = new_first as usize;
        flip_count += 1;
      }

      checksum += if perm_index % 2 == 0 {
        flip_count as i64
      } else {
        -(flip_count as i64)
      };

      if flip_count > max_flips {
        max_flips = flip_count;
      }
    }

    if perm_index >= perm_max - 1 {
      break;
    }

    perm_index += 1;

    // next permutation — a factorial-radix increment of `count`.
    let mut first_value = current[1];
    current[1] = current[0];
    current[0] = first_value;

    let mut i = 1usize;
    while count[i] >= i as i32 {
      count[i] = 0;
      i += 1;

      let new_first = current[1];
      current[0] = new_first;

      for j in 1..i {
        current[j] = current[j + 1];
      }

      current[i] = first_value;
      first_value = new_first;
    }

    count[i] += 1;
  }

  (checksum, max_flips)
}

fn main() {
  let n: usize = env::args()
    .nth(1)
    .and_then(|a| a.parse().ok())
    .unwrap_or(7);

  let (checksum, max_flips) = fannkuch(n);

  println!("{}\nPfannkuchen({}) = {}", checksum, n, max_flips);
}
