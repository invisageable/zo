# arrays

An ordered, **homogeneous** sequence of values — every element shares the same type. Two flavors: fixed-size when the length is part of the type, dynamic when it grows at runtime.

## static arrays

  ```zo
  -- `[N]T`: length is part of the type. Layout is
  -- known at compile time; bounds-checked when the
  -- index is constant.
  imu nums: [3]int = [10, 20, 30];

  showln(nums[0]);       -- 10
  showln(nums.len);      -- 3

  -- `[v...]` fills with `v`. Count is read from 
  -- `[N]T`.
  imu zeros: [5]int = [0...]; -- [0, 0, 0, 0, 0]

  -- destructure by ordered position. Use `:=` —
  -- types come from the rhs.
  imu [a, b, c] := nums;
  showln(a);             -- 10
  ```

## multi-dimensional static arrays

  ```zo
  -- `[N][M]T` is an array of N arrays of M T's. Index
  -- with chained brackets.
  imu grid: [2][3]int = [[1, 2, 3], [4, 5, 6]];

  showln(grid[0][0]);    -- 1
  showln(grid[1][2]);    -- 6
  showln(grid);          -- [[1, 2, 3], [4, 5, 6]]
  ```

## dynamic arrays

  ```zo
  -- `[]T`: length lives in the runtime header, not in
  -- the type. With `mut`, the array grows.
  mut arr: []int = [];

  arr.push(10);
  arr.push(20);
  arr.push(30);

  showln(arr);           -- [10, 20, 30]
  showln(arr.len);       -- 3

  imu last: int = arr.pop();
  showln(last);          -- 30
  showln(arr);           -- [10, 20]

  -- `[v...n]` is the explicit-count repeat — useful
  -- with `[]T` since there's no annotation length.
  imu sevens: []int = [7...4]; -- [7, 7, 7, 7]
  ```

## under the hood

zo arrays are length-prefixed: `[len:8][cap:8][elem0][elem1]...[elemN]`. The header is 16 bytes (length + capacity); element offsets start at `+16`. `arr.len` is a single load — no walk, no terminator. Indexing is constant-offset arithmetic. `push`/`pop` mutate in place; reallocation kicks in when capacity is exhausted.

  ```zo
  -! ## the capstone.
  -!
  -!   - `[N]T` is fixed-size, `[]T` is dynamic.
  -!   - `a[i]` reads, `a.len` counts (O(1)).
  -!   - `[v...]` / `[v...n]` are repeat literals.
  -!   - `imu [a, b, c] := arr` destructures.
  -!   - `mut` enables `push` / `pop` / resize.
  ```
