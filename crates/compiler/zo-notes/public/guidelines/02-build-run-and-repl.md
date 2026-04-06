# build, run and repl.

> *Mastering the zo tools ecosystem.*

## commands.

**-programming-mode**

`build` YOUR PROGRAM AS AN EXECUTABLE. GO TO `crates/compiler/zo-tests/programming` AND RUN:

  ```sh
  zo -- build hello.zo -o hello
  ```

> *It will creates an executable named `hello` in the current folder.*

THEN, YOU CAN RUN iT:

  ```sh
  ./hello
  ```

iT WiLL PRiNTS:

  ```
  hello, world!
  ```

**-templating-mode**

`run` YOUR PROGRAM iN A NATiVE WiNDOW<sup>`winit`</sup> OR A LiGHTWEiGHT WEBViEW<sup>`wry`</sup> DEPENDiNG OF THE TARGET. `crates/compiler/zo-tests/templating` AND RUN:

  ```sh
  zo -- run zsx-hello.zo
  ```

THAT'S iT, YOU SHOULD SEE A NATiVE APP ON YOUR SCREEN.

> *This command is only for `native` app, if you want to build for `web` you should add the `--web` flag.*

<p align="center">
  <img width="340" src="./zo-notes/public/preview/preview-zo-hello-template-native.png" />
  <img width="340" src="./zo-notes/public/preview/preview-zo-hello-template-web.png" />
</p>

**-repl-(wip)**

RUN AN EXECUTABLE ENViRONMENT:

  ```sh
  zo repl
  ```

> *Run an zo environment to play with the language in a sandbox.*

---

[prev](./01-install.md) — [next](./00-prologue.md)
