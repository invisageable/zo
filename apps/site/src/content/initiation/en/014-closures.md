# closures

  ```zo
  -- closure:block.
  imu square: Fn(int) -> int = fn(x: int) -> int {
    return x * x;
  };

  -- closure:line.
  imu square: Fn(int) -> int = fn(x: int) -> int => x * x;


  -- Then you can call me, like this:
  f(7);
  ```
