# variables

Data bindings demand structured tracking. zo enforces variable clarity using three dedicated allocation keywords: `val` for global or local compile-time constants, `imu` for unalterable execution parameters, and `mut` for active stack transformations.

## constants

  ```zo
  -- Hi, I'm `val` — your compile-time constant.
  val VERSION: str = "1.0.0";
  val MAX_HEALTH: int = 100;
  ```

## locals

  ```zo
  -- Hey, I'm `imu` — your immutable local. Set me 
  -- once, read me many.
  imu name: str = "johndoe";

  -- And me, I'm `mut` — your mutable local. I can be
  -- reassigned.
  mut health: int = 22;
  health = 50; -- Legal modification mutation.
  ```

## shadowing

  ```zo
  -- Variable shadowing isolates structural scopes.
  -- Each statement allocates a fresh slot, shadowing
  -- the predecessor safely.
  imu x: int = 40;
  imu x: int = x + 2; -- x now safely evaluates to 42.
  ```
