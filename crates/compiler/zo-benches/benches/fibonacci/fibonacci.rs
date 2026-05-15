fn fib(n: i64) -> i64 {
  if n == 0 {
    return 0;
  } else {
    if n <= 2 {
      return 1;
    } else {
      return fib(n - 1) + fib(n - 2);
    }
  }
}

fn main() {
  assert_eq!(fib(8), 21);
  assert_eq!(fib(15), 610);
  println!("{}", fib(8));
  println!("{}", fib(15));
}
