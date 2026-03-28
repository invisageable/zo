# 07 — the inventory.

a hero carries items. each item has a name and a power level. you need
a way to group related data. that's `struct`.

```zo
struct Item {
  name: str,
  power: int,
}

fun main() {
  imu sword := Item { name: "iron sword", power: 12 };

  showln("equipped: {sword.name}");
  showln("power: {sword.power}");
}
```

```
equipped: iron sword
power: 12
```

`struct` groups named fields into one type. access fields with `.`.

let's build a proper hero:

```zo
struct Hero {
  name: str,
  hp: int,
  atk: int,
  def: int,
}

fun show_hero(h: Hero) {
  showln("=== {h.name} ===");
  showln("hp:  {h.hp}");
  showln("atk: {h.atk}");
  showln("def: {h.def}");
}

fun main() {
  imu hero := Hero {
    name: "arya",
    hp: 100,
    atk: 15,
    def: 8,
  };

  show_hero(hero);
}
```

```
=== arya ===
hp:  100
atk: 15
def: 8
```

structs are passed to functions like any other value. `h: Hero` means
the function takes a `Hero`.

## try this.

- add a `level: int` field to `Hero`.
- create a `Potion` struct with `name` and `heal_amount`.
- write a function `describe_item(item: Item)` that prints the item.

## what you learned.

- `struct` groups named fields into a type.
- create instances with `Type { field: value }`.
- access fields with `.` — `hero.name`, `hero.hp`.
- pass structs to functions like any other type.
