fn ack(m: i64, n: i64) -> i64 {
  if m == 0 {
    return n + 1;
  } else {
    if n == 0 {
      return ack(m - 1, 1);
    } else {
      return ack(m - 1, ack(m, n - 1));
    }
  }
}

fn main() {
  assert_eq!(ack(0, 0), 1);
  assert_eq!(ack(3, 2), 29);
  assert_eq!(ack(3, 4), 125);
}