# fret Refactor Plan

## Goal

Split the monolithic `fret` crate into focused sub-crates under
`crates/packager/`, mirroring the zo compiler pattern:

```
zo/src/main.rs  → zo_driver::run()  → Driver → Compiler
fret/src/main.rs → fret_driver::run() → Driver → Pipeline
```

The `fret` crate becomes a **binary-only** crate with just `main.rs`.

---

## Phase 0: Fix Stale Examples

Two examples use the **old** `package name { key = value }` syntax.
Update to the current `@pack = (key: value,)` format before any
structural changes.

| File | Issue |
|------|-------|
| `examples/build_project.rs:52-56` | `package hello { name = "hello" ... }` |
| `examples/benchmark_pipeline.rs:74-80` | `package benchmark { name = "benchmark" ... }` |

**Action:** Rewrite inline config strings to `@pack = (...)`.

---

## Phase 1: `fret-tokens`

**Source:** `fret/src/token.rs`
**Destination:** `crates/packager/fret-tokens/src/lib.rs`

Contents:
- `TokenKind` enum
- `Token` struct + methods (`new`, `lexeme`, `is_empty`, `len`)
- `Display` impl for `TokenKind`

Zero dependencies. Leaf crate.

---

## Phase 2: `fret-tokenizer`

**Source:** `fret/src/lexer.rs` (rename `Lexer` → `Tokenizer`)
**Destination:** `crates/packager/fret-tokenizer/src/lib.rs`

Contents:
- `Tokenizer` struct (renamed from `Lexer`)
- `next_token`, whitespace/comment skipping, string/number/ident
- Unit tests

Dependencies: `fret-tokens`

---

## Phase 3: `fret-types`

**Source:** `fret/src/types.rs`
**Destination:** `crates/packager/fret-types/src/lib.rs`

Contents:
- `BuildContext`, `ProjectConfig`, `Version`
- `CompilerFlags`, `BuildMode`, `Target`
- `Stage` trait, `StageError`, `PipelineStage`
- `BuildArtifact`

Dependencies: none (only `std`)

---

## Phase 4: `fret-parser`

**Source:** `fret/src/parser.rs`
**Destination:** `crates/packager/fret-parser/src/lib.rs`

Contents:
- `Parser` struct + recursive descent logic
- `ParseError` struct
- `parse_config` public function
- `unescape_string`, `parse_version`, `parse_string_array`
- Unit tests

Dependencies: `fret-tokens`, `fret-tokenizer`, `fret-types`

---

## Phase 5: `fret-pipeline`

**Source:** `fret/src/pipeline.rs` + `fret/src/stage/` (all 6 files)
**Destination:** `crates/packager/fret-pipeline/src/`

Structure:
```
fret-pipeline/src/
  lib.rs          — pub mod pipeline, stage; re-exports
  pipeline.rs     — Pipeline struct + PipelineError
  stage.rs        — mod declarations + re-exports
  stage/
    load_config.rs
    collect_sources.rs
    compile.rs
    generate_plan.rs
    execute_plan.rs
    resolve_dependencies.rs
```

All stages stay together — they're tightly coupled through
`BuildContext` mutation and the `Stage` trait.

Dependencies:
- `fret-parser` (for `parse_config`)
- `fret-types` (for `BuildContext`, `Stage`, etc.)
- `zo-compiler`, `zo-codegen-backend`
- `hashbrown`, `rayon`

---

## Phase 6: `fret-driver`

**New crate.** Mirrors `zo-driver` pattern.
**Destination:** `crates/packager/fret-driver/src/`

Structure:
```
fret-driver/src/
  lib.rs          — pub fn run() { Driver::parse().run(); }
  driver.rs       — clap::Parser, routes to Cmd variants
  cmd.rs          — Handle trait + Cmd enum
  cmd/
    build.rs      — Cmd::Build → Pipeline::simple_mode().execute()
    init.rs       — Cmd::Init  → scaffold new project
```

Commands (initial):
- `fret build [path]` — build a project (calls fret-pipeline)
- `fret init [name]`  — create new project skeleton

Dependencies:
- `fret-pipeline`
- `fret-types` (for Target, BuildMode)
- `clap` (CLI parsing)

---

## Phase 7: `fret` Becomes Binary

**Source:** rewrite `fret/src/lib.rs` → delete, create `main.rs`
**Pattern:** identical to `zo/src/main.rs`

```rust
fn main() {
  fret_driver::run();
}
```

No lib.rs. No re-exports. Just `main.rs`.

`Cargo.toml` changes:
- Remove all direct deps except `fret-driver`
- Remove `[lib]` section if present
- Keep `[[bin]]` or let default apply
- Move examples to `fret-driver` or `fret-pipeline`

---

## Dependency Graph

```
fret-tokens          (leaf, zero deps)
fret-types           (leaf, zero deps)
    |
    v
fret-tokenizer       (fret-tokens)
    |
    v
fret-parser          (fret-tokens, fret-tokenizer, fret-types)
    |
    v
fret-pipeline        (fret-parser, fret-types,
    |                  zo-compiler, zo-codegen-backend,
    v                  hashbrown, rayon)
fret-driver          (fret-pipeline, fret-types, clap)
    |
    v
fret                 (binary: fret-driver only)
```

---

## Workspace Changes

Add to root `Cargo.toml`:

```toml
# members + default-members
"crates/packager/fret-tokens",
"crates/packager/fret-tokenizer",
"crates/packager/fret-types",
"crates/packager/fret-parser",
"crates/packager/fret-pipeline",
"crates/packager/fret-driver",

# [workspace.dependencies]
fret-tokens = { path = "crates/packager/fret-tokens", version = "0.0.0" }
fret-tokenizer = { path = "crates/packager/fret-tokenizer", version = "0.0.0" }
fret-types = { path = "crates/packager/fret-types", version = "0.0.0" }
fret-parser = { path = "crates/packager/fret-parser", version = "0.0.0" }
fret-pipeline = { path = "crates/packager/fret-pipeline", version = "0.0.0" }
fret-driver = { path = "crates/packager/fret-driver", version = "0.0.0" }
```

---

## Where Things Move

| Current Location | New Home |
|-----------------|----------|
| `fret/src/token.rs` | `fret-tokens/src/lib.rs` |
| `fret/src/lexer.rs` | `fret-tokenizer/src/lib.rs` |
| `fret/src/types.rs` | `fret-types/src/lib.rs` |
| `fret/src/parser.rs` | `fret-parser/src/lib.rs` |
| `fret/src/pipeline.rs` | `fret-pipeline/src/pipeline.rs` |
| `fret/src/stage.rs` | `fret-pipeline/src/stage.rs` |
| `fret/src/stage/*.rs` | `fret-pipeline/src/stage/*.rs` |
| `fret/src/lib.rs` | **deleted** |
| `fret/src/main.rs` | **new**: `fret_driver::run()` |
| `fret/build.rs` | `fret-pipeline/build.rs` |
| `fret/examples/` | `fret-pipeline/examples/` |
| `fret/tests/` | split: parser tests → `fret-parser`, pipeline tests → `fret-pipeline` |
| `fret/my-project/` | `fret-pipeline/tests/fixtures/my-project/` |

---

## Execution Order

1. Fix stale examples (Phase 0)
2. Create leaf crates: `fret-tokens`, `fret-types` (Phases 1, 3)
3. Create `fret-tokenizer` (Phase 2)
4. Create `fret-parser` (Phase 4)
5. Create `fret-pipeline` (Phase 5)
6. Create `fret-driver` (Phase 6)
7. Rewrite `fret` as binary-only (Phase 7)
8. Update workspace `Cargo.toml`
9. Move tests, examples, fixtures
10. `just pre-commit` to verify
