# fret — parser.

## about.

hand-written recursive descent parser for the fret.oz configuration format. transforms tokens into `ProjectConfig` with clear error messages and source location reporting.

## features.

- `@pack = (key: value, ...)` directive parsing
- namespaced directives (`@pack:zo:release`)
- string escape sequences (`\n`, `\t`, `\"`, `\\`)
- string arrays (`["a", "b"]`)
- version parsing (`"major.minor.patch"`)
- error snippets with line/column pointers
