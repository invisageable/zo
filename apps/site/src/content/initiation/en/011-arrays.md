# arrays

Arrays host homogeneous components where every single block matches a identical data type. They come in static and dynamic variants.

## static array

The notation format `[N]T` fuses the length constraint straight into the data classification layer. The memory block is determined at compile-time and guarantees safe bounds-checking verification parameters during constant array access tasks.

  ```zo
  imu nums: [3]int = [10, 20, 30];
  imu zeros: [5]int = [0...]; -- [0, 0, 0, 0, 0]
  imu grid: [2][3]int = [[1, 2, 3], [4, 5, 6]];

  -- Unpack items instantly using assignment sequences.
  imu [a, b, c]: [int, int, int] = nums;
  ```

## dynamic array

The designation `[]T` offloads length calculations to execution headers rather than structural types. Coupling
arrays with a `mut` binding permits array extensions.

  ```zo
  mut arr: []int = [];
  arr.push(10);

  imu last: int = arr.pop(); -- Safely extracts 10

  -- The `[value...count]` expression triggers explicit
  -- array expansion routines.
  imu sevens: []int = [7...4]; -- [7, 7, 7, 7]
  ```
