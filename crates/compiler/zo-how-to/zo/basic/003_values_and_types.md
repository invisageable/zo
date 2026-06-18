This program declares one value per type family. `int` and `float` are the defaults you reach for; the sized variants exist for when you need an exact width.

| family   | default | sized variants          |
| :------- | :------ | :---------------------- |
| signed   | `int`   | `s8` `s16` `s32` `s64`  |
| unsigned | `uint`  | `u8` `u16` `u32` `u64`  |
| float    | `float` | `f32` `f64`             |
| boolean  | `bool`  |                         |
| text     | `str`   | `char` `bytes`          |

> *`int` is `s32`, `uint` is `u32`, `float` is `f64`. Annotate a sized type only when the layout matters — otherwise the defaults are right.*