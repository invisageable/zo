# zo-for — JavaScript (devs).

**-literals**

| literals    | javascript                                        | zo                                                 |
| :---------- | :------------------------------------------------ | :------------------------------------------------- |
| integers    | `4`                                               | `4`                                                |
| floats      | `1.234`                                           | `1.234`                                            |
| identifiers | `camelCase`, `PascalCase`, `SCREAMING_SNAKE_CASE` | `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE` |
| chars       | `"lorem ipsum"`, `'lorem ipsum'`                  | `'\0'`                                             |
| strings     | `"lorem ipsum"`, `'lorem ipsum'`                  | `"lorem ipsum"`                                    |
| booleans    | `false`, `true`                                   | `false`, `true`                                    |

**-arrays**

| arrays              | javascript               | zo                      |
| :------------------ | :----------------------- | :---------------------- |
| array               | `[1, 2, 3]`              | `[1, 2, 3]`             |
| array-access        | `foo[0]`                 | `foo[0]`                |
| array-destructuring | `const [x, y] = [0, 1];` | `imu [x, y] := [0, 1];` |

**-tuples**

| tuples              | javascript            | zo                      |
| :------------------ | :-------------------- | :---------------------- |
| tuple               | *no tuples supported* | `(1, 2, 3)`             |
| tuple-access        | *no tuples supported* | `bar.0`                 |
| tuple-destructuring | *no tuples supported* | `imu [x, y] := [0, 1];` |
