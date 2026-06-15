# abstracts

Abstracts establish zo's architecture for ad-hoc polymorphism. You define a structural contract once, then
declare custom implementation tracks across varying types via explicit implementation mappings: `apply Abstract for TargetType`.

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
      return self.x.to_str() ++ ", " ++ self.y.to_str();
    }
  }
  ```

## using abstracts as parameters.

Functions that accept elements under abstract contract bounds choose between three compilation techniques balancing raw execution speed against binary sizing limits.

### form 1 — implicit-mono.

  ```zo
  fun render(item: Display) -> str {
    item.display()
  }
  ```

The compiler manages the heavy lifting under the hood: it automatically generates a hidden type variable, extracts parameters straight from the call site, and generates a dedicated, static monomorphized copy of the function block per unique type. Zero runtime cost, zero vtable tracking lookups. Violating limits triggers error diagnostics immediately.

### form 2 — explicit-mono.

  ```zo
  fun render<$T: Display>(item: $T) -> str {
    item.display()
  }
  ```

This follows the exact same performance-optimal static compilation track as Form 1. Use this explicit syntax strategy whenever you must reuse the type constraint identifier across signature bounds—such as enforcing matched input types or coordinating return paths.

### form 3 — dynamic dispatch.

  ```zo
  fun render(item: any Display) -> str {
    item.display()
  }
  ```

Prepend the type parameter with any to box the instances safely behind a uniform vtable layout pointer. The compiler generates exactly one execution block in the final binary, executing code paths via runtime lookup addresses. This trade-off incurs slight call overhead but permits heterogeneous data grouping inside shared array vectors.

  ```zo
  mut widgets: []any Drawable = [];
  widgets.push(Button { label = "ok" });
  widgets.push(Slider { value = 42 });

  for w := widgets {
    showln(w.draw());
  }
  ```

| Engineering Requirement                                             | Optimal Architectural Choice        |
| :------------------------------------------------------------------ | :---------------------------------- |
| Maximum dispatch velocity; isolated single types per call site.     | `item: Abstract (Form 1)`           |
| Shared type bounds enforcement across arguments or return paths.    | `<$T: Abstract>(item: $T) (Form 2)` |
| Mixed collection handling; minimal compiled binary space footprint. | `item: any Abstract (Form 3)`       |
