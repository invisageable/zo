# 04 — choose your path.

the hero reaches a fork. left goes to the forest, right to the cave.
decisions need `if/else`.

```zo
fun main() {
  imu courage := 7;

  if courage >= 5 {
    showln("you enter the dark cave.");
    showln("a faint glow ahead...");
  } else {
    showln("you take the forest path.");
    showln("birds sing overhead.");
  }
}
```

```
you enter the dark cave.
a faint glow ahead...
```

you can chain decisions with `else if`:

```zo
fun main() {
  imu hp := 40;

  if hp > 75 {
    showln("you feel strong.");
  } else if hp > 25 {
    showln("you've seen better days.");
  } else {
    showln("barely standing...");
  }
}
```

```
you've seen better days.
```

for quick one-liners, zo has `when` — a ternary expression:

```zo
fun main() {
  imu hp := 40;
  imu status := when hp > 50 ? "healthy" : "wounded";

  showln("status: {status}");
}
```

```
status: wounded
```

`when cond ? a : b` — if `cond` is true, the whole expression
evaluates to `a`. otherwise `b`. clean and compact.

## try this.

- add a third `else if` branch for `hp > 50`.
- use `when` to set a damage multiplier: `1` if `atk > 10`, else `2`.
- nest an `if` inside another `if` — check courage *and* hp.

## what you learned.

- `if / else if / else` for branching.
- `when cond ? a : b` for ternary expressions.
- comparison operators: `==`, `!=`, `>`, `<`, `>=`, `<=`.
