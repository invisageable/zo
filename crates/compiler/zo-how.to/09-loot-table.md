# 09 — loot table.

a dungeon has many chests. you need a collection of items. arrays hold
a fixed list of values of the same type.

```zo
fun main() {
  imu loot: int[] = [10, 25, 50, 5, 100];

  showln("=== chest contents ===");
  showln("first: {loot[0]} gold");
  showln("last: {loot[4]} gold");
  showln("total items: 5");
}
```

```
=== chest contents ===
first: 10 gold
last: 100 gold
total items: 5
```

`int[]` means "array of ints". access elements with `[index]`.
indices start at 0.

combine arrays with loops to process all items:

```zo
fun main() {
  imu rewards: int[] = [10, 25, 50, 5, 100];

  mut total := 0;

  for i := 0..5 {
    showln("chest #{i}: {rewards[i]} gold");
    total += rewards[i];
  }

  showln("=== total: {total} gold ===");
}
```

```
chest #0: 10 gold
chest #1: 25 gold
chest #2: 50 gold
chest #3: 5 gold
chest #4: 100 gold
=== total: 190 gold ===
```

use array indexing with arithmetic:

```zo
fun main() {
  imu dmg: int[] = [5, 12, 8, 20, 3];

  -- compare two hits.
  imu first := dmg[0];
  imu second := dmg[1];
  imu combined := first + second;

  showln("combo damage: {combined}");
}
```

```
combo damage: 17
```

## try this.

- create an array of 3 monster HPs and loop through them.
- find the max value in an array using a `while` loop and `if`.
- use `check@eq(arr[2] * arr[3], 12)` to verify array math.

## what you learned.

- `type[]` declares an array type: `int[]`, `str[]`.
- `[a, b, c]` creates an array literal.
- `arr[i]` accesses element at index `i` (0-based).
- combine arrays with `for` loops to process collections.
