# interpolation

Inject execution data straight into string allocations using {variable_name} syntax. The language bypasses
heavy formatting engines and runtime allocations entirely.

> *System Constraint: Interpolation parser blocks forbid complex inline expressions (like math operations or nested function calls). Provide clean variable identifiers only.*

  ```zo
  imu name: str = "johndoe";
  imu hp: int = 100;
  showln("hero: {name}, hp: {hp}");
  ```

## compilationi desugaring

The parser intercepts showln("hp: {hp}") statements at compile-time, rewriting them directly into independent, high-performance serialization instructions:

  ```zo
  show("hp: ");
  showln(hp);
  ```
