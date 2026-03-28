# 06 — the bestiary.

your game has one monster. boring. you need different kinds. that's
what `enum` is for — a type that can be one of several variants.

```zo
enum Monster {
  Goblin,
  Skeleton,
  Dragon,
}

fun main() {
  imu enemy := Monster::Goblin;
}
```

each variant is a distinct value. `Monster::Goblin` is not
`Monster::Dragon`. the `::` syntax accesses a variant from its enum.

variants can carry data:

```zo
enum Monster {
  Goblin(int),
  Skeleton(int),
  Dragon(int),
}

fun main() {
  imu boss := Monster::Dragon(500);
}
```

here, the `int` inside each variant is the monster's HP. one type,
many shapes — that's the power of enums.

a simpler example — representing what the hero finds in a room:

```zo
enum Loot {
  Gold(int),
  Potion(int),
  Nothing,
}

fun main() {
  imu drop := Loot::Gold(50);
  imu empty := Loot::Nothing;
}
```

`Nothing` has no data. `Gold(50)` carries a value. both are `Loot`.

## try this.

- add a `Boss(int, int)` variant with HP and ATK.
- create an enum `Weapon` with `Sword`, `Bow`, `Staff`.
- make a `Direction` enum: `North`, `South`, `East`, `West`.

## what you learned.

- `enum` defines a type with distinct variants.
- variants can carry data: `Variant(type)`.
- `::` accesses enum variants: `Enum::Variant`.
- unit variants (no data) are like named constants.
