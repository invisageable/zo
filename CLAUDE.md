# zo Compiler Ecosystem

## Mission

Build the `zo` compiler using execution-based compilation (Carbon/Chandler Carruth approach) to achieve **10,000,000 lines of code per second** AOT compilation. Systems, not features. Performance, simplicity, total control.     

The grammar can be found here: @crates/compiler/zo-notes/public/grammar/zo.ebnf     

---

## The Three Prime Directives

These laws are absolute. They supersede all other instructions.

1. **THE LAW OF VELOCITY:** The synchronous `compile-to-run` path is sacrosanct. Reject any complexity that threatens 10M LoC/s. The user must **never** be blocked by work not essential to producing a runnable binary.

2. **THE LAW OF PRAGMATISM:** No "magic." No incremental compilation, complex caching, or esoteric IRs. Velocity comes from **brute-force parallelism and linear data transformations**. Own the entire stack with hand-written, data-oriented components. Proven techniques (Hindley-Milner) over theoretical experiments (bidirectional type systems).

3. **THE LAW OF INSIGHT:** Deep analysis is critical but **must never violate Law I**. The `Copilord` runs asynchronously in the background, consuming immutable AOT artifacts. The user's flow is paramount.

---

## Architecture

### Two Pipelines

- **AOT Pipeline** (`Parse -> Tree -> Execute/Analyze -> SIR -> Codegen`): Synchronous, stateless, from-scratch. Purpose: speed.
- **Copilord Pipeline** (`SIR -> Scan -> Suggest`): Asynchronous, background. Purpose: insight.
- **One-Way Data Flow:** AOT generates artifacts for Copilord. Copilord **never** feeds back into AOT.

### Parallelism Model

- **Hybrid Parallelism:** Central MPSC Scheduler orchestrates; `rayon` thread pool executes.
- **Waves:** Parse Wave -> Lowering Wave -> Codegen Wave. A wave completes before the next begins.
- All inter-thread data must be `Send + Sync`.

### Data Sovereignty

- All processes are data transformations: `Source Text -> Tokens -> Tree -> SIR -> Machine Code`.
- **SIR** (Semantic IR) is the most critical artifact — typed, optimized output of executing Tree. Feeds both codegen and Copilord.
- **Tree** (via `zo-tree`) is the parse tree — simple, fast, no types, no analysis. Exists only to be executed into SIR.
- Favor stack allocation, arenas, and zero-allocation strategies (especially tokenizer/parser).

---

## Execution-Based Compilation Protocol

We do **NOT** use traditional AST -> TypeCheck -> IR phases.

- **Pipeline:** `Parse -> Tree (parse tree) -> Execute/Analyze -> SIR (semantic IR) -> Codegen`
- **Core concept:** Semantic analysis is **compile-time execution** that produces IR, not tree annotation.
- Type checking happens **during** SIR building, not as a separate pass.

### Directives

1. **Execution model, not tree walking.** The analyzer "executes" Tree to produce SIR. Declarations are compile-time operations. Type checking is evaluation.

2. **Linear data flow.** Tree nodes processed sequentially as an array. Types flow through a stack machine. SIR emitted immediately as types resolve.

3. **Single pass semantics.** Tree executed once to produce typed SIR. No multiple tree walks.
   ```rust
   // NOT THIS: walk tree -> annotate -> walk again -> generate IR
   // BUT THIS: execute Tree -> emit SIR (with types) in one pass
   ```

4. **Streaming architecture.** Can start emitting SIR without complete type information. Types resolved incrementally. Functions execute independently.

### Performance Targets

| Phase | Target | Benchmark |
|-------|--------|-----------|
| Tokenize + Parse (-> Tree) | **10M LoC/s** | Carbon: 8M |
| Semantic Analysis (Tree -> SIR) | **5M LoC/s** | Carbon: 1M |
| Codegen (SIR -> machine code) | **5M LoC/s** | excl. LLVM |

---

## Code Style

1. **Indentation**: 2 spaces (no tabs).
2. **Line width**: 80 characters max (per `rustfmt.toml`).
3. **File operations**: Exclude read/write in `tmp/` folder.
4. **Bug fixes**: Never mark fixed without testing the solution.
5. **Warnings**: Never ignore — they often indicate bugs or incomplete code.
6. **Root causes**: Fix the cause, not silence the symptom.
7. **Understand before removing**: Know why code exists before deleting it.

## Build System

All build commands go through `just` (the justfile is the single source of truth):

```
just typos         # Check for typos
just fmt           # Format code
just fmt_check     # Check formatting without modifying
just clippy        # Clippy with -D warnings
just test          # Run all tests (nextest)
just test_crate X  # Test a single crate
just test_filter X # Test by name filter
just build         # Build all targets
just bench         # Run all benchmarks
just pre-commit    # Full pipeline: typos -> fmt_check -> clippy -> test
```

Pre-commit hooks via `lefthook` run the same pipeline automatically. Always use `just` recipes — never raw cargo commands.
