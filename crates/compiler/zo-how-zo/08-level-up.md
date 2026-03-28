# 08 — level up.

your hero struct has data. now give it *behavior*. `apply` attaches
methods to a type.

```zo
struct Hero {
  name: str,
  hp: int,
  atk: int,
  def: int,
}

apply Hero {
  fun new(name: str, hp: int, atk: int, def: int) -> Self {
    Self { name, hp, atk, def }
  }

  fun power(self) -> int {
    self.hp + self.atk + self.def
  }

  fun show(self) {
    showln("=== {self.name} ===");
    showln("hp:  {self.hp}");
    showln("atk: {self.atk}");
    showln("def: {self.def}");
    showln("power: {self.power()}");
  }
}

fun main() {
  imu hero := Hero::new("arya", 100, 15, 8);
  hero.show();
}
```

```
=== arya ===
hp:  100
atk: 15
def: 8
power: 123
```

two kinds of methods:

- **static** — no `self`, called with `::`. `Hero::new(...)` creates
  a hero.
- **instance** — takes `self`, called with `.`. `hero.show()` acts on
  an existing hero.

`Self` inside `apply` refers to the type being applied. so
`Self { name, hp, atk, def }` is the same as
`Hero { name, hp, atk, def }`.

## try this.

- add an `is_alive(self) -> bool` method that returns `self.hp > 0`.
- create a `Weapon` struct with an `apply` block and a `damage(self)`
  method.
- add a `level_up(self) -> Self` that returns a new hero with
  `atk + 5`.

## what you learned.

- `apply Type { ... }` attaches methods to a struct.
- `Self` = the type being applied.
- static methods: no `self`, called with `::`.
- instance methods: take `self`, called with `.`.
