# structures

Custom structures package operations into meaningful domain boundaries. The `struct` keyword models complex custom fields, while the `apply` keyword assigns functional behavior logic to those types.

## struct

A field can declare a default value with `=`. Fields without one are set when you construct the value.

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
