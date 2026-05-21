# variables

Data needs a name. In zo, you have three ways to name a value:
`val` for constants, `imu` for immutable locals, and `mut` when things change.

Pick the least powerful one that works — that's the zo way.

## constants

  ```zo
  -- Hi, I'm `val` — your compile-time constant.
  val VERSION: str = "1.0.0";
  val MAX_HEALTH: int = 100;
  -- I work at the top level or inside a function.
  ```

## locals

  ```zo
  -- Hey, I'm `imu` — your immutable local. Set me 
  -- once, read me many.
  imu name: str = "johndoe";

  -- And me, I'm `mut` — your mutable local. I can be
  -- reassigned. Reach for me only when something 
  -- changes.
  mut health: int = 22;
  health = 50; -- Reassignment.
  ```

## shadowing

  ```zo
  -- Call me `shadowing` — I let you re-declare a name
  -- in the same scope. Each declaration creates a new
  -- variable; the old one is untouched.
  imu x: int = 40;
  imu x: int = x + 1; -- shadows; new x is 41
  imu x: int = x + 1; -- shadows again; new x is 42
  ```

In this lesson, we wrote every type explicitly (like `: int`). In practice, zo can almost always figure it out. We'll stay explicit for now to help you learn, but soon we'll let the compiler do the heavy lifting in `Inference`.

