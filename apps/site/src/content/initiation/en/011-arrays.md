# arrays

An ordered, **homogeneous** sequence of values — every element shares the same type. Two flavors: fixed-size when the length is part of the type, dynamic when it grows at runtime.

## static array ([N]T)

  ```zo
  -- `[N]T`: length is part of the type. Layout is
  -- known at compile time; bounds-checked when the
  -- index is constant.
  imu nums: [3]int = [10, 20, 30];
  imu zeros: [5]int = [0...]; -- [0, 0, 0, 0, 0]
  imu grid: [2][3]int = [[1, 2, 3], [4, 5, 6]];

  -- destructure by ordered position. Use `:=` —
  -- types come from the rhs.
  imu [a, b, c]: [int, int, int] = nums;
  ```

## dynamic array ([]T)

  ```zo
  -- `[]T`: length lives in the runtime header, not in
  -- the type. With `mut`, the array grows.
  mut arr: []int = [];
  arr.push(10);

  imu last: int = arr.pop();
  showln(last);          -- 10

  -- `[v...n]` is the explicit-count repeat — useful
  -- with `[]T` since there's no annotation length.
  imu sevens: []int = [7...4]; -- [7, 7, 7, 7]
  ```

  ```zo
  -! ## the capstone.
  -!
  -!   - `[N]T` is fixed-size, `[]T` is dynamic.
  -!   - `a[i]` reads, `a.len` counts (O(1)).
  -!   - `[v...]` / `[v...n]` are repeat literals.
  -!   - `imu [a, b, c] := arr` destructures.
  -!   - `mut` enables `push` / `pop` / resize.
  ```
