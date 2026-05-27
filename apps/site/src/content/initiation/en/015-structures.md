# structures

Custom structures package operations into meaningful domain boundaries. The `struct` keyword models complex custom fields, while the `apply` keyword assigns functional behavior logic to those types.

## struct

Every field inside a `struct` definition must declare an explicit fallback assignment metric. The compiler enforces default initialization rules to maintain optimizations throughout compilation phases.

  ```zo
  struct Point {
    x: int,
    y: int,
  }

  struct Rect {
    x: int = 10,
    y: int = 20,
    w: int = 100,
    h: int = 200,
  }

  struct Counter {
    x: int = 0,
  }
  ```

## methods

Use the apply statement block to bind custom functions to a target structure definition.

  ```zo
  apply Counter {
    -- Static instantiation function block.
    fun new() -> Self {
      Self { x = 0 }
    }

    -- Mutable state tracking modifier function.
    fun incr(mut self) {
      self.x += 1;
    }
  }
  ```
