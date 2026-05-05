# operators

Operators are how you transform values. zo keeps them small and predictable — the same five arithmetic operators you've used everywhere, plus reassignment and a handful of shorthands.

## arithmetic

  ```zo
  -- "Hi, we're the arithmetic operators — `+`, `-`, `*`, `/`, `%`.
  -- Same precedence and associativity as math: multiply and divide bind
  -- tighter than add and subtract."
  imu hp: int = 100;
  imu attack: int = 15;

  imu power: int = hp + attack;   -- 115
  imu damage: int = attack * 2;   -- 30
  imu rest: int = 10 % 3;         -- 1 (modulo: remainder)
  showln("power: {power}, damage: {damage}, rest: {rest}");
  ```

## reassignment

  ```zo
  -- Reassign a `mut` with a plain `=`. The variable's type stays fixed;
  -- only its value changes.
  mut current_hp: int = 100;
  imu damage: int = 25;

  current_hp = current_hp - damage;
  showln("hp: {current_hp}"); -- 75
  ```

## compound assignment

  ```zo
  -- "And me, I'm compound assignment — `+=`, `-=`, `*=`, `/=`. Just
  -- shorthand for `x = x + y`. Read-and-write in one shot."
  mut current_hp: int = 100;

  current_hp -= 25; -- damage
  showln("hp: {current_hp}"); -- 75

  current_hp += 30; -- heal
  showln("hp: {current_hp}"); -- 105

  current_hp *= 2;  -- berserk
  showln("hp: {current_hp}"); -- 210
  ```

## try this

- Use `%` to detect even numbers: `showln(10 % 2);`.
- Try `current_hp /= 0;` and watch the compiler/runtime react.

```zo
-! ## the capstone.
-!
-!   - arithmetic: `+ - * / %` — same rules as math.
-!   - reassignment: `=` — only on `mut` variables.
-!   - compound: `+= -= *= /=` — shorter spelling, same result.
```
