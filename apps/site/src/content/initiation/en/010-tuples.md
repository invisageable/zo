# tuples

A tuple organizes items into a fixed-length, ordered sequence. Unlike arrays, a single tuple can house distinct
data types within the same memory footprint.

  ```zo
  imu point: (int, str, int) = (100, "john", 3);

  -- Extract parameters via positional indices.
  showln(point.0);

  -- Deconstruct the memory layer instantly via
  -- structured binding patterns.
  imu (x, y, z): (int, int, int) = point; -- Structured destructuring binding

  -- Declare structural type aliases for fast shape
  -- replication.
  type Point = (int, int); -- Type shaping alias
  ```
