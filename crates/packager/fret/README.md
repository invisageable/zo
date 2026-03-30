# fret.

> *The high-performance package manager for zo.*

```js
@pack = (
  name: "my-project",
  version: "0.1.0",
  authors: ["you <you@example.com>"],
  license: "MIT",
)
```

## about.

FRET iS AN HiGH-PERFORMANCE BUiLD SYSTEM AND PACKAGE MANAGER FOR THE zo PROGRAMMiNG LANGUAGE.


iT FOLLOWS THE SAME ARCHiTECTURE AS zo iTSELF — A THiN BiNARY THAT DELEGATES TO A DRiVER.

## architecture.

```
fret (binary)
  -> fret-driver (cli, command routing)
    -> fret-pipeline (build orchestration, stages)
      -> fret-parser (recursive descent parser)
        -> fret-tokenizer (zero-allocation tokenizer)
        -> fret-tokens (token types)
      -> fret-types (core data structures)
```

## crates.

| crate            | role                                 |
| :--------------- | :----------------------------------- |
| `fret`           | binary entry point                   |
| `fret-driver`    | cli parsing (clap), command dispatch |
| `fret-pipeline`  | build pipeline, all stages           |
| `fret-parser`    | fret.oz config parser                |
| `fret-tokenizer` | zero-allocation tokenizer            |
| `fret-tokens`    | token types                          |
| `fret-types`     | core types, traits, errors           |

## usage.

```sh
fret build [path]    # build a zo project
fret init <name>     # create a new zo project
```

## configuration.

projects are configured via `fret.oz`:
