# 10 — quick reflexes.

different weapons have different damage formulas. you could write a
function for each. or you could store the formula itself as a value.
that's a closure.

```zo
fun main() {
  imu sword_dmg := fn(atk: int) -> int => atk * 2;
  imu bow_dmg := fn(atk: int) -> int => atk + 5;
  imu staff_dmg := fn(atk: int) -> int => atk * 3;

  imu atk := 10;

  showln("sword: {sword_dmg(atk)}");
  showln("bow: {bow_dmg(atk)}");
  showln("staff: {staff_dmg(atk)}");
}
```

```
sword: 20
bow: 15
staff: 30
```

`fn(params) => expr` is an inline closure. it's a function without a
name, stored in a variable. call it like any other function.

closures are values. pass them around:

```zo
fun apply_damage(atk: int, formula: Fn(int) -> int) -> int {
  formula(atk)
}

fun main() {
  imu crit := fn(atk: int) -> int => atk * 4;

  imu result := apply_damage(15, crit);
  showln("critical hit: {result}");
  check@eq(result, 60);
}
```

```
critical hit: 60
```

`Fn(int) -> int` is the type of a closure that takes an `int` and
returns an `int`. functions can accept closures as parameters.

you can also assign a named function to a variable:

```zo
fun double(x: int) -> int {
  return x * 2;
}

fun main() {
  imu f: fn() -> int = double;
  showln(f(21));
}
```

```
42
```

## try this.

- create a `heal` closure: `fn(hp: int) -> int => hp + 30`.
- write a function that takes two closures and applies both.
- make a closure that captures an outer variable.

## what you learned.

- `fn(params) => expr` creates an inline closure.
- closures are values — store them, pass them, call them.
- `Fn(type) -> type` is the closure type signature.
- named functions can also be assigned to variables.
