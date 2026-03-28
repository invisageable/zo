# 13 — boss fight.

the dragon guards the treasure. it has a pattern: it attacks in waves,
each wave stronger than the last. to model this, you need recursion —
a function that calls itself.

```zo
fun dragon_attack(wave: int) -> int {
  if wave <= 1 {
    return 10;
  } else {
    return dragon_attack(wave - 1) + 15;
  }
}

fun main() {
  showln("=== dragon attack pattern ===");
  showln("wave 1: {dragon_attack(1)} damage");
  showln("wave 2: {dragon_attack(2)} damage");
  showln("wave 3: {dragon_attack(3)} damage");
  showln("wave 5: {dragon_attack(5)} damage");
}
```

```
=== dragon attack pattern ===
wave 1: 10 damage
wave 2: 25 damage
wave 3: 40 damage
wave 5: 70 damage
```

each call to `dragon_attack` calls itself with a smaller number until
it hits the base case (`wave <= 1`). the results stack up on the way
back.

you can also use indirect function calls — store a function in a
variable and call it later:

```zo
fun heal(hp: int) -> int {
  return hp + 30;
}

fun poison(hp: int) -> int {
  return hp - 20;
}

fun main() {
  imu effect: fn(int) -> int = heal;
  imu hp := 50;

  imu result := effect(hp);
  showln("after effect: {result}");
  check@eq(result, 80);
}
```

```
after effect: 80
```

the dragon boss, in full:

```zo
fun dragon_breath(wave: int) -> int {
  if wave <= 1 {
    return 10;
  } else {
    return dragon_breath(wave - 1) + 15;
  }
}

fun main() {
  mut hero_hp := 200;
  mut wave := 1;

  showln("=== boss fight: dragon ===");

  while hero_hp > 0 {
    imu dmg := dragon_breath(wave);
    hero_hp -= dmg;

    showln("wave {wave}: dragon deals {dmg}. hp: {hero_hp}");

    wave += 1;
  }

  showln("the dragon wins at wave {wave - 1}.");
}
```

## try this.

- write a recursive `fibonacci(n)` — classic puzzle, works as an XP
  formula.
- make the hero fight back: reduce dragon HP each wave too.
- swap `effect` between `heal` and `poison` based on a condition.

## what you learned.

- recursion: a function calling itself with a smaller input.
- every recursive function needs a **base case** to stop.
- functions are values — assign them to variables, pass them around.
- `fn(type) -> type` is the function pointer type.
