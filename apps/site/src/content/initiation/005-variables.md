# variables

Data needs a name. In zo, you have three ways to name a value:
`val` for constants, `imu` for immutable locals, and `mut` when things change.

Pick the least powerful one that works — that's the zo way.

## constants

  ```zo
  -- "Hi, I'm `val` — your compile-time constant. I never change, and I
  -- need an explicit type (you can't infer me). The compiler inlines me
  -- everywhere I'm used: no stack slot, no runtime cost. I work at the top
  -- level or inside a function — anywhere a name is welcome."
  val VERSION: str = "1.0.0";
  val MAX_HEALTH: int = 100;

  fun main() {
    showln(VERSION);
  }
  ```

## locals

### immutables

  ```zo
  -- "Hey, I'm `imu` — your immutable local. Set me once, read me many.
  -- I'm the default for 90% of your variables; I make code easier to read."
  imu name: str = "johndoe";
  showln(name);
  ```

### mutables

  ```zo
  -- "And me, I'm `mut` — your mutable local. I can be reassigned. Reach
  -- for me only when something genuinely changes over time."
  mut health: int = 22;
  showln(health);

  -- Reassign a `mut` like this:
  health = 50;
  showln(health);
  ```

## shadowing

  ```zo
  -- "Call me `shadowing` — I let you re-declare a name in the same scope.
  -- Each declaration creates a new variable; the old one is untouched."
  imu x: int = 40;
  imu x: int = x + 1; -- shadows; new x is 41
  imu x: int = x + 1; -- shadows again; new x is 42
  showln(x);          -- 42
  ```

In this lesson, we wrote every type explicitly (like `: int`). In practice, you'll rarely need to. zo can almost always figure it out. We'll stay explicit for now to help you learn, but soon we'll let the compiler do the heavy lifting in `Inference`.

```zo
-! ## when to use what?
-!
-!   `val` — Global constant. Known before the program even runs.
-!   `imu` — Immutable local. Set once, read many. (The Default).
-!   `mut` — Mutable local. Changes over time.
```
