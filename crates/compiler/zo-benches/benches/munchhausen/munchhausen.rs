const MAX: i64 = 440_000_000;

fn pow_int(b: i64, e: i64) -> i64 {
  let mut result = 1;
  for _ in 0..e {
    result *= b;
  }
  result
}

fn is_munchhausen(n: i64, pwr: &[i64]) -> bool {
  let mut sum = 0;
  let mut temp = n;
  while temp > 0 {
    sum += pwr[(temp % 10) as usize];
    temp /= 10;
  }
  sum == n
}

fn main() {
  let mut pwr = [0i64; 10];
  for i in 0..10 {
    pwr[i as usize] = pow_int(i, i);
  }
  for n in 1..=MAX {
    if is_munchhausen(n, &pwr) {
      println!("{n}");
    }
  }
}
