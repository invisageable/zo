# crates — compiler.

## about.

@see: [zo](../compiler//zo/) to get more information.

EVERYTHiNG REGARDiNG THE zo COMPiLER MUST BE PLACED HERE.

## commandes.

**programming**

```bash
cargo run --bin zo -- build crates/compiler/zo-tests/build-pass/programming/hello.zo -o crates/compiler/zo-tests/build-pass/programming/hello
```

**templating**

```bash
cargo run --bin zo -- run crates/compiler/zo-tests/build-pass/templating/zsx-hello.zo
```

> this command is only for `native` app, if you want to build for `web` you should add the `--web` flag.
