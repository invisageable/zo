# zo-for — Python (devs).

**-literals**

| literals    | python                                             | zo                                                 |
| :---------- | :------------------------------------------------- | :------------------------------------------------- |
| integers    | `4`                                                | `4`                                                |
| floats      | `1.234`                                            | `1.234`                                            |
| identifiers | `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE` | `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE` |
| chars       | ...                                                | `'\0'`                                             |
| strings     | `"lorem ipsum"`, `'lorem ipsum'`                   | `"lorem ipsum"`                                    |
| booleans    | `false`, `true`                                    | `false`, `true`                                    |

**-arrays**

| arrays              | python            | zo                      |
| :------------------ | :---------------- | :---------------------- |
| array               | `[1, 2, 3]`       | `[1, 2, 3]`             |
| array-access        | `foo[0]`          | `foo[0]`                |
| array-destructuring | `[x, y] = [0, 1]` | `imu [x, y] := [0, 1];` |

**-tuples**

| tuples              | python            | zo                      |
| :------------------ | :---------------- | :---------------------- |
| tuple               | `(1, 2, 3)`       | `(1, 2, 3)`             |
| tuple-access        | `bar[0]`          | `bar.0`                 |
| tuple-destructuring | `(x, y) = (0, 1)` | `imu (x, y) := (0, 1);` |
