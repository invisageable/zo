# zo.

> *The `zo` programming language.*

## compiler phases.

- reading.
- tokenizing.
- parsing.
- analyzing.
- interpreting.
- building.

## syntax.

```rs
-- a constant variable.
val A: bool = true;

-- a simple fibonacci.

fun main() {
  imu fib := fn (n) -> when n < 2 
    ? 1
    : fib(n - 1) + fib(n - 2);

  println("{}", fib(11));
}
```
