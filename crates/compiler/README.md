# compiler.

> *The `zhoo` compiler.*

## compiler phases.

```
                                          |-- bytecode:arm:asm?:(wat, wasm).
                                          |-- code:py. 
                                          |
source -> tokn -> prse -> anlz -> itrp -> genr.
                                  |
                                  |-- repl:vm?:clif(jit).
                                  |-- repl:wasmtime.
```
