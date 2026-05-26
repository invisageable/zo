# tuples

A tuple is an ordered, fixed-length collection of items. Unlike arrays or vectors, you can group different types together by concepts.

  ```zo
  imu point: (int, str, int) = (100, "john", 3);
  showln(point.0); -- Tuple index extraction
  imu (x, y, z): (int, int, int) = point; -- Structured destructuring binding

  type Point = (int, int); -- Type shaping alias
  ```
