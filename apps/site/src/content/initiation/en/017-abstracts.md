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

## using abstracts as parameters.

A function that accepts "any type implementing `Display`" has
three forms. Each picks a different dispatch strategy.

### form 1 — implicit-mono `item: Abstract`.

  ```zo
  fun render(item: Display) -> str {
    item.display()
  }
  ```

The parameter type names the abstract directly. The compiler
synthesizes a fresh type parameter under the hood, infers it
from the call site, and emits one **monomorphized** copy of
`render` per concrete type. Each call dispatches statically
— no vtable, no allocation. Calling `render` with a non-
implementing type fires [E0347](#100-error-messages)
(`BoundNotSatisfied`) at the call site.

### form 2 — explicit-mono `<$T: Abstract>(item: $T)`.

  ```zo
  fun render<$T: Display>(item: $T) -> str {
    item.display()
  }
  ```

Same monomorphization model as form 1. Use this form when you
need to reference `$T` elsewhere in the signature
(`-> $T`, `(left: $T, right: $T)`) — the explicit name makes
the same-type constraint visible. Multi-bound is supported:
`<$T: Display + Eq>(item: $T)`.

### form 3 — dynamic dispatch `item: any Abstract`.

  ```zo
  fun render(item: any Display) -> str {
    item.display()
  }
  ```

The parameter type prefix `any` boxes the value behind a
**vtable**. One copy of `render` ships in the binary; the
method call resolves through the vtable at runtime. Slower
per call than mono dispatch, but supports heterogeneous
collections:

  ```zo
  mut widgets: []any Drawable = [];

  widgets.push(Button { label = "ok" });
  widgets.push(Slider { value = 42 });

  for w := widgets {
    showln(w.draw());
  }
  ```

Not every abstract is dyn-safe. An abstract that uses `Self`
outside the receiver position — for example
`fun merge(self, other: Self) -> Self` — cannot ride a
vtable because the calling convention has no slot for
"another instance of the same concrete type". The compiler
reports [E0349](#100-error-messages) (`AbstractNotDynSafe`)
at the `any Abstract` annotation site; switch to the implicit
or explicit mono form, or drop the `Self`-using method.

## choosing a form.

| need | use |
| :--- | :--- |
| fastest dispatch, one type per call site | `item: Abstract` (form 1) |
| same-type constraint across params or return | `<$T: Abstract>(item: $T)` (form 2) |
| heterogeneous collection, one body in the binary | `item: any Abstract` (form 3) |

Abstracts are flat single-level declarations. `abstract X : Y`
inheritance is not supported — express the relationship as
parallel `apply Parent for Type` blocks instead. The
compiler raises [E0348](#100-error-messages)
(`AbstractInheritanceUnsupported`) at the offending colon.
