# 02 — name your hero.

a hero needs a name. to remember a name, you need a variable.

```zo
fun main() {
  imu name := "arya";
  imu title := "shadow knight";

  showln("=========================");
  showln("  hero: {name}");
  showln("  class: {title}");
  showln("=========================");
}
```

```
=========================
  hero: arya
  class: shadow knight
=========================
```

`imu` means immutable — it can't change once set. good for things that
stay fixed, like a name.

`:=` is the binding operator. it says "this name refers to this value."

`{name}` inside a string is interpolation — zo drops the value right
into the text. no format functions, no concatenation.

## try this.

- add a `level` variable: `imu level := 1;`
- print it: `showln("  level: {level}");`
- try changing `name` after declaring it — what happens?

## what you learned.

- `imu` creates an immutable binding.
- `:=` binds a name to a value.
- `"text {var}"` interpolates variables into strings.
- `str` is the string type.
- `int` is the integer type.
