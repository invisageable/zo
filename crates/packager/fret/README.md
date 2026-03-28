# fret.

> *the blazing fast package manager for zo.*

```js
@pack = (
  name: "my-project",
  version: "0.1.0",
  authors: ["you <you@example.com>"],
  license: "MIT",
)
```

## about.

fret is the build system and package manager for the zo programming language. it follows the same architecture as zo itself: a thin binary that delegates to a driver.

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
