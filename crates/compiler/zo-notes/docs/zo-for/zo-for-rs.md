# zo-for — Rust (devs).

**-literals**

| literals    | rust                                               | zo                                                 |
| :---------- | :------------------------------------------------- | :------------------------------------------------- |
| integers    | `4`                                                | `4`                                                |
| floats      | `1.234`                                            | `1.234`                                            |
| identifiers | `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE` | `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE` |
| chars       | `'\0'`                                             | `'\0'`                                             |
| strings     | `"lorem ipsum"`                                    | `"lorem ipsum"`                                    |
| booleans    | `false`, `true`                                    | `false`, `true`                                    |

**-arrays**

| arrays              | rust                   | zo                      |
| :------------------ | :--------------------- | :---------------------- |
| array               | `[1, 2, 3]`            | `[1, 2, 3]`             |
| array-access        | `foo[0]`               | `foo[0]`                |
| array-destructuring | `let [x, y] = [0, 1];` | `imu [x, y] := [0, 1];` |

**-tuples**

| tuples              | rust                   | zo                      |
| :------------------ | :--------------------- | :---------------------- |
| tuple               | `(1, 2, 3)`            | `(1, 2, 3)`             |
| tuple-access        | `bar.0`                | `bar.0`                 |
| tuple-destructuring | `let (x, y) = (0, 1);` | `imu (x, y) := (0, 1);` |

**-variables**

| variables                               | rust                      | zo                    |
| :-------------------------------------- | :------------------------ | :-------------------- |
| constants                               | `const OOF: usize = 4`    | `val OOF: int = 4;`   |
| immutable local variable — *typed*      | `let rab: usize = 23;`    | `imu rab: int = 23;`  |
| immutable local variable — *inferenced* | `let rab = 23;`           | `imu rab := 23;`      |
| mutable local variable — *typed*        | `let mut foo: f32 = 1.5;` | `mut foo: f32 = 1.5;` |
| mutable local variable — *inferenced*   | `let mut foo = 1.5;`      | `mut foo := 1.5;`     |
