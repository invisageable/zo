# zhoo-driver.

> *The `command-line interface` of the compiler.*

## commands.

- [build](./src/cmd/build.rs) — *compiles and builds a package depending of the choosen target.*
- [check](./src/cmd/check.rs) — *todo.*
- [license](./src/cmd/license.rs) — *todo.*
- [print](./src/cmd/print.rs) — *todo.*
- [version](./src/cmd/version.rs) — *todo.*

### commands arguments.

| command   | arguments                                        | description                          |
| --------- | ------------------------------------------------ | ------------------------------------ |
| `build`   | `--input <pathname>`, `--backend <backend_kind>` | *compile a `zhoo` program*           |
| `check`   | `--input <pathname>`, `--backend <backend_kind>` | *analyze a `zhoo` program*           |
| `license` | `N/A`                                            | *show the `zhoo` license*            |
| `print`   | `--input <pathname>`, `--backend <backend_kind>` | *display a specific compiler output* |
| `version` | `N/A`                                            | *show the `zhoo` version*            |

### commands details.

- `<pathname>` — *...*
- `<backend_kind>` — *`arm`, `asm`, `clif`, `js`, `llvm`, `py` and `wasm`.*
- `<output>` — *`--bytes`, `--tokens`, `--ast`, `--hir`, `--mir`, `--ir`, `--bytecode`.*

### commands flags.

- `--profile` — *show the time spend by each compiler phase.*
- `--verbose` — *show the logs.*
