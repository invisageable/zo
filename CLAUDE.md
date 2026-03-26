# zo Compiler Ecosystem

## Mission

Build the `zo` compiler using execution-based compilation (Carbon/Chandler Carruth approach) to achieve **10,000,000 lines of code per second** AOT compilation. Systems, not features. Performance, simplicity, total control.

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


---

  What works end-to-end (compile + run):
  - Functions with int arithmetic — binaries execute correctly
  - Multi-function programs — add(10, 20) returns 30
  - Local variable arithmetic — imu x = 1; imu y = 2; x + y returns 3
  - Register allocation — Graydon-style linear scan, dual GP/FP pools,
  liveness analysis, spilling, function prologue/epilogue
  - Module system — load/pack, resolution, compilation, symbol merging
  - DCE — removes uncalled functions
  - Implicit return — fun foo() -> int { 42 } works
  - Error reporting — `:` return type syntax reports ExpectedArrow
  - String output — showln("hello world!") prints correctly
  - _main entry point — resolves to correct function offset in Mach-O
  - zo build and zo run share the same analysis pipeline
  - Control flow — if/else branches work end-to-end at runtime
  - While loops — mutable variables use stack-based storage, loop-carried
  state works (sum of 0..5 returns 10)
  - For loops — `for i := 0..5 { body }` desugars to while loop SIR
  - Float parameter passing — D0-D7 for float args, FMOV for FP moves
  - break/continue — compiles to Jump targeting loop end/start labels
  - Arrays — [1, 2, 3] literals, arr[i] indexing (stack-allocated)
  - showln(str/int/float) — compile-time type dispatch (Graydon style),
  inline itoa for ints, ftoa (integer.fractional) for floats
  - `ext` keyword — declares intrinsic functions with no body
  (pub ext readln() -> str;), used by std modules
  - String interpolation — showln("{x}, {y}") desugars to show() calls
  at compile time, zero runtime allocation
  - Ternary expressions — `when cond ? true_expr : false_expr`
  compiles to BranchIfNot/Jump/Label (same SIR as if/else)
  - Closures — inline (`fn(x) => expr`) and block (`fn(x) { body }`)
  with capture analysis, recursive closures via letrec
  - Tuples — `(1, 2)` literals, `t.0` field access, arithmetic on
  tuple elements
  - Enum declarations — `enum Foo { Ok(int), Err(int) }` with unit,
  tuple, and explicit discriminant variants
  - Enum construction — `Foo::Ok(42)` via `::` access, stack-allocated
  tagged unions (tag + payload)
  - Struct declarations — `struct Span { lo: int, hi: int }` with
  typed fields and optional default values
  - Struct construction — `Span { lo: 0, hi: 10 }`, field access
  via `s.lo`, stack-allocated sequential slots
  - Apply blocks — `apply Foo { fun new() -> Self { ... } }` associates
  methods with types. Static calls (`Foo::new()`), instance calls
  (`s.sum()`) with `self` receiver. `Self` resolves to applied type.
  - Type annotation unification — `imu x: int = 42` validates
  annotation against init expression type
  - check@eq — `check@eq(expr, value)` with nested function calls,
  closure calls, and ternary results

  What's broken or incomplete:
  - main() should only return unit (like Rust) — fun main() -> int works by
  accident (OS reads X0 as exit code) but is not correct language semantics
  - showln for enums/structs — needs `abstract` trait system for Display
  - `&self`/`&mut self` — needs reference/borrow system

  Verdict: MVP-ready. The pipeline is complete and correct: source -> tokens ->
  tree -> SIR -> ARM64 -> Mach-O binary -> runs. Int/float arithmetic, function
  calls, local/mutable variables, if/else, while, for loops, string output, and
  register allocation all work end-to-end.
