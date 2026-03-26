# 14 — fibonacci shrine.

deep in the dungeon, a locked door. the inscription reads:

> *"speak the 8th number of the golden sequence."*

the hero needs to compute fibonacci numbers. but more importantly,
*you* need to verify the answer is correct. that's what `check` and
`check@eq` are for.

```zo
fun fib(n: int) -> int {
  if n == 0 {
    return 0;
  } else if n <= 2 {
    return 1;
  } else {
    return fib(n - 1) + fib(n - 2);
  }
}

fun main() {
  -- the shrine demands proof.
  check@eq(fib(1), 1);
  check@eq(fib(2), 1);
  check@eq(fib(5), 5);
  check@eq(fib(8), 21);

  showln("the answer is {fib(8)}.");
  showln("the door opens.");
}
```

```
the answer is 21.
the door opens.
```

`check@eq(a, b)` asserts that `a` equals `b`. if it doesn't, the
program fails immediately. it's your proof that the code is correct.

plain `check` works with any boolean:

```zo
fun main() {
  imu hero_hp := 50;
  imu min_hp := 10;

  -- make sure the hero is alive.
  check(hero_hp > min_hp);

  imu damage := 8;
  imu expected := 42;

  check@eq(hero_hp - damage, expected);

  showln("all checks passed.");
}
```

```
all checks passed.
```

use `check` liberally. it catches bugs before they become mysteries.

a math puzzle for the hero — verify factorial too:

```zo
fun fact(n: int) -> int {
  if n <= 1 {
    return 1;
  } else {
    return n * fact(n - 1);
  }
}

fun main() {
  check@eq(fact(1), 1);
  check@eq(fact(5), 120);
  check@eq(fact(7), 5040);

  showln("factorial shrine cleared.");
}
```

```
factorial shrine cleared.
```

## try this.

- add `check@eq(fib(11), 89)` and `check@eq(fib(15), 610)`.
- write a `gcd` (greatest common divisor) function and verify it.
- deliberately break a check to see the error message.

## what you learned.

- `check(condition)` asserts a boolean is true.
- `check@eq(a, b)` asserts two values are equal.
- failed checks stop the program immediately.
- use checks to prove your code is correct.
