# interpolation

Numbers are useless if you can't show them. In zo, you drop values directly into strings with `{variable}` — no format functions, no concatenation. 

> *Interpolation does not allow expressions, like binop, function calls, etc. It needs the variable name.*

  ```zo
  -- Hi, I'm `{}` — a string interpolation. Wrap any 
  -- in-scope variable in braces and I expand it 
  -- inline at compile time.
  imu name: str = "johndoe";
  imu hp: int = 100;
  showln("hero: {name}, hp: {hp}");
  ```

## multiple values

  ```zo
  -- Interpolate as many as you like in one go.
  imu x: int = 10;
  imu y: int = 12;
  showln("position: {x}, {y}");
  ```

## under the hood

`showln("hp: {hp}")` is NOT a runtime format call. The compiler desugars it:

  ```zo
  show("hp: ");
  showln(hp);
  ```

Three things this buys you: zero allocations, zero runtime parsing, zero surprises. As fast as writing the calls by hand.

## try this

  - Interpolate a variable that hasn't been declared. Watch the compiler protect you.
  - Interpolate a `float` and a `bool` — any primitive works.

  ```zo
  -! ## the capstone.
  -!
  -!   - any in-scope variable can be interpolated.
  -!   - works for any primitive type.
  -!   - desugared at compile time — no runtime cost.
  ```
