# 05 — the grind.

combat is a loop. you hit the monster, the monster hits you. repeat
until someone drops.

```zo
fun main() {
  mut hero_hp := 100;
  mut monster_hp := 50;
  imu hero_atk := 18;
  imu monster_atk := 12;
  mut turn := 1;

  while monster_hp > 0 {
    showln("--- turn {turn} ---");

    monster_hp -= hero_atk;
    showln("you deal {hero_atk} damage. monster hp: {monster_hp}");

    if monster_hp <= 0 {
      showln("the monster falls!");
      break;
    }

    hero_hp -= monster_atk;
    showln("monster strikes! your hp: {hero_hp}");

    turn += 1;
  }

  showln("=== battle over ===");
}
```

```
--- turn 1 ---
you deal 18 damage. monster hp: 32
monster strikes! your hp: 88
--- turn 2 ---
you deal 18 damage. monster hp: 14
monster strikes! your hp: 76
--- turn 3 ---
you deal 18 damage. monster hp: -4
the monster falls!
=== battle over ===
```

`while condition { body }` repeats until the condition is false.
`break` exits the loop early.

zo also has `for` loops with ranges:

```zo
fun main() {
  showln("=== loot drops ===");

  for i := 0..5 {
    showln("chest #{i}");
  }
}
```

```
=== loot drops ===
chest #0
chest #1
chest #2
chest #3
chest #4
```

`0..5` produces values from 0 to 4. the upper bound is exclusive.

## try this.

- add a win/lose condition: if `hero_hp <= 0`, print "you died."
- use `continue` to skip a turn when `turn % 3 == 0` (the monster
  flinches).
- use a `for` loop to run 10 battles in a row.

## what you learned.

- `while condition { ... }` loops until false.
- `for x := start..end { ... }` iterates a range.
- `break` exits a loop early.
- `continue` skips to the next iteration.
