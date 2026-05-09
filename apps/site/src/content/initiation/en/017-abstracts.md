# abstracts

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
      return "point";
    }
  }
  ```
