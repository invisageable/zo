# tuples

A fixed-size group of values, possibly of different types, accessed by position. Heterogeneous, immutable, zero-overhead grouping — no heap, no headers.

  ```zo
  -- A tuple literal: parens + comma-separated values.
  -- The type follows the values: `(int, int, int)`.
  imu point := (1, 2, 3);
  showln(point); -- (1, 2, 3)
  ```

## indexing

  ```zo
  -- `.N` reads the Nth field. Positional, 0-indexed.
  imu tup := (10, 20);
  showln(tup.0);       -- 10
  showln(tup.1);       -- 20
  showln(tup.0 + t.1); -- 30
  ```

## type annotations

  ```zo
  -- The type mirrors the literal: parens around
  -- comma-separated types. Mix types freely.
  imu pair: (int, int) = (10, 20);
  imu mixed: (str, int) = ("alice", 30);
  ```

## destructuring

  ```zo
  -- Bind each field to its own name in one shot.
  imu (x, y, z) := (1, 2, 3);
  showln(x); -- 1
  showln(y); -- 2
  showln(z); -- 3

  imu (name, age) := ("alice", 30);
  showln(name); -- alice
  showln(age);  -- 30
  ```

## naming a shape

  ```zo
  -- `type` names a recurring tuple — clearer 
  -- signatures, same zero-cost layout.
  type Point = (int, int);

  fun translate(p: Point, dx: int, dy: int) -> Point {
    return (p.0 + dx, p.1 + dy);
  }

  imu origin: Point = (0, 0);
  imu next: Point = translate(origin, 5, 5); -- (5, 5)
  ```

## under the hood

A tuple is a flat struct with positional fields: `[field0][field1]...[fieldN]`, no headers, no padding beyond alignment. Field access (`t.N`) is a constant-offset load — same cost as struct field access. Tuples pass by value: `imu b = a` copies the fields.

  ```zo
  -! ## the capstone.
  -!
  -!   - tuples group values of different types.
  -!   - `t.N` reads field N (positional, 0-indexed).
  -!   - `imu (a, b) := t` destructures.
  -!   - `type Pair = (int, int)` names a shape.
  ```
