# 01 — press start.

every game starts with a title screen. every zo program starts with `main`.

```zo
fun main() {
  showln("=========================");
  showln("    dungeons of zo       ");
  showln("=========================");
  showln("      press start.       ");
}
```

it will prints:

```
=========================
    dungeons of zo       
=========================
      press start.       
```

`fun` declares a function. `main` is where your program begins.`showln` prints a line to the screen. that's it. you made a game. well, the start of one.

## try this.

- change the game title to something else.
- add more lines to the title screen.
- use `--` to add a comment above the function:

```zo
-- i'm cooking some programming stuff.
fun main() {
  showln("dungeons of zo");
}
```

## what you learned.

- `fun` declares a function.
- `main()` is the entry point.
- `showln(...)` prints text.
- `--` starts a comment.
