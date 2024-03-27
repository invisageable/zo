function fibonacci(n) {
  return n < 2 ? n : fibonacci(n - 2) + fibonacci(n - 1);
}

const fibonacci = (n) => {
  if (n < 2) return n;

  return fibonacci(n - 1) + fibonacci(n - 2);
}
