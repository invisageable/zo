# abstracts

Abstracts are zo's form of **ad-hoc polymorphism** — one contract, many implementations. Each `apply Abstract for Type` block provides type-specific behavior dispatched on the
receiver's type. The other form, **parametric polymorphism**,
is covered by [generics](#019-generics).

  ```zo
  abstract Display {
    fun display(self) -> str;
  }

  struct Point {
    x: int,
    y: int,
  }

  apply Display for Point {
    fun display(self) -> str {
      return self.x ++ ", " ++ self.y;
    }
  }
  ```
