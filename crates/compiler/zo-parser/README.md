# zo-parser.

> *The `syntax analysis` phase.*

## about.

The `parser` receives a bunch of tokens. These tokens are then used by the `parser` module, which constructs an abstract syntax tree (`ast`).   

## dirmap.

```
src                       # ...
|-- lib.rs                # import all the files related by the `parser`.
|-- parser.rs             # the instance of the parser.
|-- precedence.rs         # the `precedence` enumeration used by the `parser`.
```
