# 11 — the map.

your hero moves on a grid. each position is an x and y coordinate.
tuples are perfect for this — lightweight pairs (or triples, or more)
of values.

```zo
fun main() {
  imu pos := (3, 7);

  showln("hero is at ({pos.0}, {pos.1})");
}
```

```
hero is at (3, 7)
```

access tuple elements with `.0`, `.1`, `.2`, etc. no names, just
positions.

tuples can hold mixed types and work as function return values:

```zo
fun move_north(x: int, y: int) -> (int, int) {
  (x, y + 1)
}

fun move_east(x: int, y: int) -> (int, int) {
  (x + 1, y)
}

fun main() {
  imu start := (0, 0);
  showln("start: ({start.0}, {start.1})");

  imu after_north := move_north(start.0, start.1);
  showln("moved north: ({after_north.0}, {after_north.1})");

  imu after_east := move_east(after_north.0, after_north.1);
  showln("moved east: ({after_east.0}, {after_east.1})");
}
```

```
start: (0, 0)
moved north: (0, 1)
moved east: (1, 1)
```

tuples are useful when you need to return multiple values from a
function without defining a full struct.

```zo
fun hero_stats() -> (int, int, int) {
  (100, 15, 8)
}

fun main() {
  imu stats := hero_stats();

  showln("hp: {stats.0}, atk: {stats.1}, def: {stats.2}");
}
```

```
hp: 100, atk: 15, def: 8
```

## try this.

- make a `distance` function using `(x1 - x2) * (x1 - x2)`.
- return a tuple `(str, int)` from a function — hero name and level.
- simulate a 3-step path using a loop and position updates.

## what you learned.

- `(a, b)` creates a tuple.
- `.0`, `.1`, `.2` access tuple elements by position.
- functions can return tuples for multiple return values.
- tuples are lighter than structs when names aren't needed.
