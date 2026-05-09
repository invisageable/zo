# operators

Operators are how you transform values. zo keeps them small and predictable — the same five arithmetic operators you've used everywhere, plus reassignment and a handful of shorthands.

## arithmetic

  ```zo
  -- Hi, we're the arithmetic operators:
  -- `+`, `-`, `*`, `/`, `%`. Same precedence and 
  -- associativity as math.
  imu hp: int = 100;
  imu attack: int = 15;

  imu power: int = hp + attack;   -- 115
  imu damage: int = attack * 2;   -- 30
  imu rest: int = 10 % 3;         -- 1 (remainder)
  ```

## reassignment

  ```zo
  -- Reassign a `mut` with a plain `=`. The variable's
  -- type stays fixed; only its value changes.
  mut current_hp: int = 100;
  imu damage: int = 25;

  current_hp = current_hp - damage; -- 75
  ```

## compound assignment

  ```zo
  -- And me, I'm compound assignment:
  -- `+=`, `-=`, `*=`, `/=`. Just shorthand for 
  -- `x = x + y`. Read-and-write in one shot.
  mut current_hp: int = 100;

  current_hp -= 25; -- 75
  current_hp += 30; -- 105
  current_hp *= 2;  -- 210
  ```

## try this

- Use `%` to detect even numbers: `showln(10 % 2);`.
- Try `current_hp /= 0;` and watch the compiler/runtime react.

```zo
-! ## the capstone.
-!
-!   - `+ - * / %` are arithmetic same rules as math.
-!   - `=` are reassignment, only on `mut` variables.
-!   - `+= -= *= /=` are compound.
```
