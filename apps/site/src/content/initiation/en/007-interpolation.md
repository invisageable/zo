# interpolation

In zo, drop values directly into strings with `{variable}` — no format functions, no allocations at runtime.

> *Interpolation does not allow full complex expressions (like binary operations or direct function calls). It requires a clean variable identifier name.*

  ```zo
  -- Hi, I'm `{}` — a string interpolation. Wrap any 
  -- in-scope variable in braces and I expand it 
  -- inline at compile time.
  imu name: str = "johndoe";
  imu hp: int = 100;
  showln("hero: {name}, hp: {hp}");
  ```

## under the hood

The compiler desugars `showln("hp: {hp}")` directly into explicit performance-optimal blocks at compile-time:

  ```zo
  show("hp: ");
  showln(hp);
  ```
