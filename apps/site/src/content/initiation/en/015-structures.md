# structures

## struct

  ```zo
  -- ...
  struct Point {
    x: int,
    y: int,
  }

  -- Then you created me:
  imu point: Point = Point {
    x = 1,
    y = 2
  };

  -- And used me like the following:
  point.y -- 2
  ```

talk about the default value. All fields must have a default value to be valid. otherwise the compiler will complain.

All structured layout definitions require standard default initial assignment metrics to maintain type validation
stability guarantees during optimization stages

  ```zo
  -- ...
  struct Rect {
    x: int = 10,
    y: int = 20,
    w: int = 100,
    h: int = 200,
  }
  ```

## methods

talk about the usage of apply keyword to define methods.

  ```zo
  struct Counter {
    x: int,
  }

  -- ...
  apply Counter {
    -- static method
    fun new() -> Self {
      Self { x = 0 }
    }

    -- ...
    fun incr(mut self) {
      self.x += 1;
    }
  }
  ```
