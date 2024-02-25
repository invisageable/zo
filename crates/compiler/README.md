# compiler.

> *...*

## compiler phases.

```
                                          |-- bytecode:arm:asm?:wasm
                                          |   |-- repl:wasmtime
                                          |
source -> tokn -> prse -> anlz -> itrp -> genr.
                                  |
                                  |-- repl:vm?:cranelift(jit)
```
