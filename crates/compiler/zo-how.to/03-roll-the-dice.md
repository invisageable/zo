# 03 — roll the dice.

every hero needs stats. let's give yours some numbers.

```zo
fun main() {
  imu hp := 100;
  imu atk := 15;
  imu def := 8;

  showln("=== hero stats ===");
  showln("hp:  {hp}");
  showln("atk: {atk}");
  showln("def: {def}");

  imu power := hp + atk + def;
  showln("power level: {power}");
}
```

```
=== hero stats ===
hp:  100
atk: 15
def: 8
power level: 123
```

zo supports the arithmetic you'd expect: `+`, `-`, `*`, `/`, `%`.

now, what if the hero takes a hit? you need a value that *changes*.
that's `mut`.

```zo
fun main() {
  mut hp := 100;
  imu damage := 25;

  showln("hp before: {hp}");

  hp = hp - damage;
  showln("hp after: {hp}");

  hp -= 10;
  showln("hp final: {hp}");
}
```

```
hp before: 100
hp after: 75
hp final: 65
```

`mut` means mutable — the value can be reassigned. use `imu` by
default. reach for `mut` only when something genuinely changes.

## try this.

- add a `crit_multiplier` and compute `atk * crit_multiplier`.
- use `%` (modulo) to check if `hp` is even: `hp % 2`.
- make the hero heal: `hp += 30;`.

## what you learned.

- `imu` = can't change. `mut` = can change.
- `+`, `-`, `*`, `/`, `%` for arithmetic.
- `=` reassigns a `mut` variable.
- `+=`, `-=` are compound assignment operators.
