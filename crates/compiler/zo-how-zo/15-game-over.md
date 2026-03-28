# 15 — game over.

every game needs a clean ending. `return` exits a function early — use
it when the outcome is decided and there's nothing left to do.

```zo
fun battle(hero_hp: int, monster_atk: int) -> str {
  if hero_hp <= 0 {
    return "you are already dead.";
  }

  if monster_atk >= hero_hp {
    return "fatal blow. game over.";
  }

  return "you survive.";
}

fun main() {
  showln(battle(0, 10));
  showln(battle(5, 20));
  showln(battle(100, 10));
}
```

```
you are already dead.
fatal blow. game over.
you survive.
```

early return keeps functions flat. no deep nesting, no else chains.
check the exit conditions first, return immediately.

let's put the whole game together — everything from the book:

```zo
-- the complete dungeon run.

struct Hero {
  name: str,
  hp: int,
  atk: int,
}

apply Hero {
  fun new(name: str, hp: int, atk: int) -> Self {
    Self { name, hp, atk }
  }

  fun is_alive(self) -> int {
    self.hp > 0
  }
}

fun combat(hero_atk: int, monster_hp: int) -> int {
  mut hp := monster_hp;
  mut turns := 0;

  while hp > 0 {
    hp -= hero_atk;
    turns += 1;
  }

  turns
}

fun score(turns: int, hp: int) -> int {
  hp * 10 + (100 - turns * 5)
}

fun main() {
  imu hero := Hero::new("arya", 100, 18);

  showln("=== dungeons of zo ===");
  showln("hero: {hero.name}");
  showln("hp: {hero.hp} | atk: {hero.atk}");
  showln("");

  -- floor 1: goblin.
  imu t1 := combat(hero.atk, 30);
  showln("floor 1: goblin defeated in {t1} turns.");

  -- floor 2: skeleton.
  imu t2 := combat(hero.atk, 50);
  showln("floor 2: skeleton defeated in {t2} turns.");

  -- floor 3: dragon.
  imu t3 := combat(hero.atk, 120);
  showln("floor 3: dragon defeated in {t3} turns.");

  imu total_turns := t1 + t2 + t3;
  imu final_score := score(total_turns, hero.hp);

  showln("");
  showln("=== game over ===");
  showln("total turns: {total_turns}");
  showln("final score: {final_score}");
  showln("well played, {hero.name}.");
}
```

## try this.

- add a 4th floor with a boss that has 200 HP.
- subtract monster damage from `hero.hp` each floor.
- add a game over screen if `hero.hp <= 0` before the final floor.
- track loot with an array and sum it at the end.

## what you learned.

- `return` exits a function early with a value.
- early returns keep code flat and readable.
- you now know: functions, variables, loops, branches, enums, structs,
  apply blocks, arrays, tuples, closures, modules, recursion, and
  checks.

**you learned zo by building a game. now go build yours.**
