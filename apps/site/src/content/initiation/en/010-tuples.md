# tuples

Fixed size, zero runtime overhead sequence records matching layout structures.

  ```zo
  imu point: (int, int, int) = (1, 2, 3);
  showln(point.0); -- Tuple index extraction
  imu (x, y, z): (int, int, int) = point; -- Structured destructuring binding

  type Point = (int, int); -- Type shaping alias
  ```
