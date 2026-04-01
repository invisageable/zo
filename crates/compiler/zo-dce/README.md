# zo — dce (dead code elimination).

> *Eliminate unreachable functions from the SIR.*

## about.

  - DEAD FUNCTiON ELiMiNATiON — *worklist-based call graph walk from roots.*
  - DEAD VARiABLE ELiMiNATiON — *symbol-level liveness, dead Store removal.*
  - DEAD iNSTRUCTiON ELiMiNATiON — *ValueId-level liveness, branch-aware, fixpoint.*
  - UNREACHABLE CODE — *removes instructions after `return`.*

## dev.

### overview.

zo-dce removes unreachable function bodies from the SIR before codegen. it operates on the flat instruction stream — no CFG needed. Called from zo-compiler after semantic analysis, before codegen.

### pipeline.

four passes, run in order from `Dce::eliminate()`:

  ```
  1. eliminate_dead_functions        — interprocedural
  2. eliminate_unreachable_after_return — intraprocedural
  3. eliminate_dead_variables         — intraprocedural (symbol liveness)
  4. eliminate_dead_instructions     — intraprocedural (value liveness)
```

### pass 1: dead function elimination.

worklist-based transitive reachability:

  ```
  1. build_function_map   — scan SIR, pair FunDef → Return ranges
  2. mark_reachable       — seed worklist with roots (main + pub)
    while worklist not empty:
      pop function, mark reachable
      scan its body for Call instructions
      add callees to worklist
  3. eliminate             — drain dead ranges in reverse order
  ```

O(F + E) where F = number of functions, E = number of call edges.

### pass 2: unreachable code after return.

single linear scan. after a `Return`, removes everything until the
next `Label`, `FunDef`, or top-level definition. O(N).

### pass 3: dead variable (Store) elimination.

uses `zo-liveness` symbol-level liveness. a `Store { name }` is
dead if the named variable is not live-out — no path from that
point ever reads the stored value before it's overwritten or the
function exits.

catches both:
  - unused variables (`imu x = 42;` never used)
  - overwritten stores (`x = 1; x = 2; showln(x);` — first dead)

iterates to fixpoint (chained dead stores).

### pass 4: dead instruction elimination.

uses `zo-liveness` for branch-aware live_out per instruction.
an instruction is dead if:

  1. it defines a `ValueId` (has `dst`)
  2. that `ValueId` is not in `live_out` at that point
  3. the instruction is pure (no side effects)

**impure instructions** (never eliminated): `Call`, `Store`,
`FieldStore`, `Directive`, `Return`.

iterates to fixpoint — removing one dead instruction may make
its operands' definitions dead. recomputes liveness each iteration.
typical depth: 1-2 iterations.

### roots.

a function is a root (always kept) if:
  - it is `main` (the entry point)
  - it is `pub` (callable from other modules)

all functions transitively reachable from any root are kept.
everything else is eliminated.

### data layout.

```rust
struct FunRange {
  name: Symbol,      // function name
  start: usize,      // index of FunDef in sir.instructions
  end: usize,        // index of Return (or last insn before next FunDef)
  pubness: Pubness,  // pub or private
}
```

function ranges are built by a single linear scan. the `end` is
found by scanning forward for `Return`, stopping at the next
`FunDef`, `StructDef`, `EnumDef`, or `ConstDef` boundary.

### integration.

```rust
// zo-compiler/src/compiler.rs:384
let main_sym = tokenization.interner.intern("main");
zo_dce::eliminate_dead_functions(&mut semantic.sir, main_sym);
```

called once after semantic analysis, before codegen. mutates
`sir.instructions` in place.

### test coverage.

```
23 tests total

basic elimination:
  removes_private_uncalled_function  — private + no calls → gone
  keeps_all_when_all_called          — all called → all kept
  empty_sir_is_noop                  — empty input
  no_functions_preserves_top_level   — non-function insns kept

transitive reachability:
  transitive_call_chain_kept   — main → a → b → c, all kept
  dead_chain_removed           — dead_a → dead_b, both removed
  diamond_call_graph           — main → left/right → leaf

pub preservation:
  pub_function_kept_even_if_uncalled      — pub root survives
  pub_function_callees_transitively_kept  — pub → helper, both kept

edge cases:
  recursive_function_kept    — fib calls itself
  mutual_recursion_kept      — ping ↔ pong cycle
  only_main_no_other_functions — single function

dead variable elimination:
  dead_var_unused_store      — Store never loaded → removed
  dead_var_overwritten_store — first Store overwritten → removed

dead instruction elimination:
  dead_insn_unused_const     — unused const removed
  dead_insn_chain            — const → add chain, all dead
  dead_insn_preserves_calls  — impure Call kept

unreachable after return:
  unreachable_after_return   — code after Return removed
  unreachable_stops_at_label — Label ends dead zone
  unreachable_empty_sir_noop — empty input

error reporting:
  warns_on_unused_function   — UnusedFunction reported
  warns_on_unused_variable   — UnusedVariable reported
  no_warning_when_all_used   — clean code → no warnings
```

### benchmarks.

```sh
cargo bench -p zo-dce --bench eliminate --quiet
```

| group | benchmark | metric |
|-------|-----------|--------|
| eliminate | 10 funs, 5 dead | ~1.8 µs |
| eliminate | 100 funs, 50 dead | ~19 µs |
| eliminate | 1000 funs, 500 dead | ~668 µs |
| scaling | 100 functions | ~5.3M funs/s |
| scaling | 500 functions | ~2.4M funs/s |
| scaling | 1000 functions | ~1.5M funs/s |

### known gaps.

  - [x] transitive reachability — worklist algorithm.
  - [x] pub function preservation — pub functions are roots.
  - [x] dead instruction elimination — ValueId liveness, branch-aware, fixpoint.
  - [ ] dead variable elimination — **disabled**. `insn_var_use` in
        `zo-liveness` only tracks `Load{Local}` but named variables are
        also referenced through `VarDef`, `ConstDef`, and other
        instructions. causes false elimination of live stores. needs
        complete var-use extraction in `zo-liveness/src/insn.rs` before
        re-enabling. code + tests in place (ignored).
  - [x] unreachable code after `return` — linear scan.
  - [x] `tests/common.rs` — shared helpers (make_sir, fun, call, fun_names).
  - [x] `benches/eliminate.rs` — criterion benchmarks (small/medium/large, scaling).
  - [ ] per-function parallelism — passes 3 and 4 iterate over
        functions independently. each function's liveness is self-
        contained. `rayon::par_iter` over function ranges within
        each pass. pass ordering stays sequential (1 → 2 → 3 → 4).
  - [ ] `tests/errors.rs` — warning tests written but **disabled** (2
        ignored). DCE warnings use `report_error` which blocks
        compilation. blocked on `Severity::Warning` in `zo-error`.
        `no_warning_when_all_used` test passes.
  - [ ] SIR spans — instructions don't carry source spans. warnings use
        `Span::ZERO`. when SIR gains span tracking, DCE warnings will
        point at source locations.
  - [ ] warning vs error severity — `zo-error` has no `Severity` level.
        DCE warnings (`UnusedFunction`, `UnusedVariable`) go through
        `report_error` and are indistinguishable from real errors. needs
        a `Severity::Warning` in `zo-error::Error` and a
        `report_warning()` in `zo-reporter` so warnings don't block
        compilation. this is a cross-cutting concern — not DCE-specific.
