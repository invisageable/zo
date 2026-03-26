# 12 — the armory.

your game is growing. everything in one file gets messy. zo lets you
split code into modules with `pack` and `load`.

`pack` declares a module. `load` brings its contents into scope.

```zo
pack weapons {
  fun sword_damage(atk: int) -> int {
    atk * 2
  }

  fun bow_damage(atk: int) -> int {
    atk + 5
  }
}

fun main() {
  load::weapons::sword_damage;
  load::weapons::bow_damage;

  imu atk := 10;

  showln("sword: {sword_damage(atk)}");
  showln("bow: {bow_damage(atk)}");
}
```

```
sword: 20
bow: 15
```

`pack weapons { ... }` defines a module called `weapons`.
`load::weapons::sword_damage` pulls `sword_damage` into the current
scope so you can call it directly.

you can nest packs:

```zo
pack game {
  pack combat {
    fun attack(atk: int, def: int) -> int {
      imu dmg := atk - def;
      when dmg > 0 ? dmg : 0
    }
  }
}

fun main() {
  load::game::combat::attack;

  imu result := attack(15, 8);
  showln("damage dealt: {result}");
}
```

```
damage dealt: 7
```

use `pub` to mark functions as visible outside their pack:

```zo
pack hero {
  pub fun create(name: str) {
    showln("hero created: {name}");
  }
}

fun main() {
  load::hero::create;
  create("arya");
}
```

## try this.

- create a `monsters` pack with functions for different monster types.
- nest a `boss` pack inside `monsters`.
- use `pub` on some functions and not others — what happens?

## what you learned.

- `pack name { ... }` declares a module.
- `load::pack::item` imports an item into scope.
- `pub` makes items visible outside their pack.
- packs can be nested for organization.
