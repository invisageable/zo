# closures

  ```zo
  -- ...
  imu f: Fn(int) -> int = fn(x: int) -> int {
    x * x
  };

  -- ...
  imu square: Fn(int) -> int = fn(x: int) -> int => x * x;


  -- Then you can call me, like this:
  square(7);
  ```
