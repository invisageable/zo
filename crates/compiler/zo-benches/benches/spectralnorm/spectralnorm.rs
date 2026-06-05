// spectralnorm (benchmarksgame): largest eigenvalue of A[i][j] =
// 1/((i+j)(i+j+1)/2 + i + 1), via 10 power-method iterations of AᵀA
// on the all-ones vector. Serial port (the harness builds with plain
// rustc, so no rayon). Reads n from argv (default 100).

use std::env;

fn a(i: usize, j: usize) -> f64 {
  1.0 / (((i + j) * (i + j + 1) / 2 + i + 1) as f64)
}

fn mult_av(v: &[f64], out: &mut [f64]) {
  for (i, slot) in out.iter_mut().enumerate() {
    let mut sum = 0.0;
    for (j, &vj) in v.iter().enumerate() {
      sum += a(i, j) * vj;
    }
    *slot = sum;
  }
}

fn mult_atv(v: &[f64], out: &mut [f64]) {
  for (i, slot) in out.iter_mut().enumerate() {
    let mut sum = 0.0;
    for (j, &vj) in v.iter().enumerate() {
      sum += a(j, i) * vj;
    }
    *slot = sum;
  }
}

fn mult_atav(v: &[f64], out: &mut [f64], tmp: &mut [f64]) {
  mult_av(v, tmp);
  mult_atv(tmp, out);
}

fn spectralnorm(n: usize) -> f64 {
  let mut u = vec![1.0f64; n];
  let mut v = vec![0.0f64; n];
  let mut tmp = vec![0.0f64; n];

  for _ in 0..10 {
    mult_atav(&u, &mut v, &mut tmp);
    mult_atav(&v, &mut u, &mut tmp);
  }

  let mut v_bv = 0.0;
  let mut vv = 0.0;
  for i in 0..n {
    v_bv += u[i] * v[i];
    vv += v[i] * v[i];
  }

  (v_bv / vv).sqrt()
}

fn main() {
  let n: usize = env::args()
    .nth(1)
    .and_then(|s| s.parse().ok())
    .unwrap_or(100);

  println!("{:.9}", spectralnorm(n));
}
