# interpolation

Drop variables into any string with `{variable_name}`. The compiler resolves each hole at compile time — no runtime parsing, no format functions.

  ```zo
  imu name: str = "johndoe";
  imu hp: int = 100;
  imu attack: int = 15;

  showln("hero: {name}, hp: {hp}");
  ```

Interpolation works in every string context — assignments, arguments, return values.

  ```zo
  imu power: int = hp + attack;
  imu status: str = "power level: {power}";

  showln(status);
  ```

All scalar types resolve automatically: `str`, `int`, `float`, `bool`, `char`.

  ```zo
  imu pi: float = 3.14;
  imu active: bool = true;
  imu label: str = "pi={pi}, ok={active}";

  showln(label);
  ```

## how it works

Interpolated strings compile into a single allocation that concatenates all segments. Each non-`str` variable converts to its string representation first, then everything merges in one pass.

  ```zo
  showln("hp: {hp}");
  -- Desugars to:
  --   show("hp: ");
  --   showln(hp);
  ```

  ```zo
  imu msg: str = "hp: {hp}";
  -- Compiles to:
  --   to_str(hp) → "100"
  --   multi_concat("hp: ", "100") → "hp: 100"
  ```

Direct output through `showln` skips the heap entirely — each segment writes straight to the file descriptor Assigned strings allocate once regardless of how many `{}` holes they contain.

  ```zo
  -! ## the capstone.
  -!
  -!   - `{variable}` inside any `"string"` resolves the variable.
  -!   - `\{` produces a literal brace, not interpolation.
  -!   - direct output (`showln`) allocates nothing.
  -!   - assigned strings allocate once, not per segment.
  -!   - all scalar types (`str`, `int`, `float`, `bool`, `char`) supported.
  ```
