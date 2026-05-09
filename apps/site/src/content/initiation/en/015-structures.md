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

talk about the default value. All fields should be default to be working.

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
    fun hello(self) -> int {
      return 12345;
    }

    -- ...
    fun incr(mut self) {
      self.x += 1;
    }

    -- ...
    fun x(self) -> int {
      self.x
    }
  }
  ```
