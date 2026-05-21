# operators

Operators are how you transform values. zo keeps them small and predictable — the same five arithmetic operators you've used everywhere, plus reassignment and a handful of shorthands.

## arithmetic

  ```zo
  imu power: int = hp + attack; -- + - * / %
  mut current_hp: int = 100;
  current_hp -= 25; -- += -= *= /=
  ```

