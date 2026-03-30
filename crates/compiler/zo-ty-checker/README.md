# zo — ty checker.

> *A Hindley-Milner type system implementation.*

## about.

  - HiNDLEY-MiLNER TYPE iNFERENCE — *complete W algorithm implementation*.
  - TYPE UNiFiCATiON — *full constraint solving with occurs check*.

## dev.

### overview.

`zo-ty-checker` is the type inference and unification engine for the zo
compiler. it implements the **W algorithm** (Damas-Hindley-Milner) and is
designed as a **toolkit** — it does not walk the parse tree itself. the
`zo-executor` owns a `TyChecker` instance and calls into it during
single-pass execution of the parse tree.

### architecture.

  ```
  zo-executor (owns TyChecker)
    │
    ├── infer_*()      ← literal/op/var inference
    ├── unify()        ← constraint solving
    ├── push/pop_scope ← lexical scoping
    ├── generalize()   ← let-polymorphism (∀α. τ)
    ├── instantiate()  ← fresh variables per use-site
    └── ty_table       ← SOA storage for compound types
  ```

the type checker is **not** a standalone pass. it integrates into the
execution-based compilation model: `Tree -> Execute/Analyze -> SIR`.
type checking happens **during** SIR building, not as a separate phase.

### data layout.

  ```rust
  TyChecker {
    tys: Vec<Ty>,                         // SOA: TyId indexes here
    intern_map: HashMap<Ty, TyId>,        // O(1) type dedup
    ty_table: TyTable,                    // compound types (Fun, Array, ...)
    substitutions: HashMap<InferVarId, TyId>, // W algorithm state
    var_levels: HashMap<InferVarId, u32>,     // level-based generalization
    ty_env: HashMap<Symbol, TyId>,            // current scope bindings
    env_undo: Vec<UndoEntry>,                 // undo log for O(1) push
    env_marks: Vec<usize>,                    // scope boundaries
    poly_schemes: HashMap<Symbol, TyScheme>,  // ∀-quantified bindings
    ty_aliases: HashMap<Symbol, TyId>,        // type X = Y
    alias_undo: Vec<UndoEntry>,              // undo log for aliases
    alias_marks: Vec<usize>,                 // alias scope boundaries
  }
  ```

**21 primitive types** are eagerly registered at construction with
deterministic `TyId`s (`TyId(0)..TyId(20)`). this is critical for
cross-module type compatibility — `translate_ty_id()` in
`zo-module-resolver` relies on identical primitive IDs across checkers.

### W algorithm status.

| rule | status | notes |
|------|--------|-------|
| **[VAR]** `Γ(x) = σ ⊢ x : inst(σ)` | done | `infer_var()` — poly lookup + instantiate, then mono fallback |
| **[LIT]** literals | done | int/float → fresh var; bool/str/char → concrete |
| **[ABS]** λ-abstraction | done | fresh var for param, infer body — driven by executor |
| **[APP]** application | done | unify `(fun_ty, arg_tys)` — driven by executor |
| **[LET]** let-polymorphism | done | `generalize()` + `instantiate()` + `bind_poly()` |

[ABS] and [APP] don't have dedicated methods on `TyChecker` because the
executor orchestrates them directly using `fresh_var()`, `unify()`, and
the type environment. the building blocks are all here.

### unification completeness.

| type constructor | unify | occurs check | free vars | substitute |
|------------------|-------|--------------|-----------|------------|
| `Infer`          | yes   | yes          | yes       | yes        |
| `Fun`            | yes   | yes          | yes       | yes        |
| `Array`          | yes   | yes          | yes       | yes        |
| `Ref`            | yes   | yes          | yes       | yes        |
| `Tuple`          | yes   | yes          | yes       | yes        |
| `Struct`         | by id | —            | —         | —          |
| `Enum`           | by id | —            | —         | —          |
| `Param`          | yes   | leaf         | leaf      | leaf       |

**struct/enum**: unification works via `repr1 == repr2` (same interned
id = same type). no structural unification needed since they are nominal.

**`Ty::Param`**: named type parameters (`$T`). unifies with same-name
params (nominal equality) and with inference variables. does not unify
with concrete types — it's a rigid variable. note: the executor
currently maps `$T` to `Ty::Infer(fresh_var)` rather than using
`Ty::Param` directly, so this is forward-looking for when named
generics need to be preserved through the pipeline.

### key algorithms.

**unification** (`unify`): Robinson's algorithm. resolves both sides
through substitutions, then pattern-matches on type constructors.
recursive for compound types (fun params, array elems, tuple elems, ref
inner). `Ty::Error` absorbs to prevent error cascading.

**occurs check** (`occurs_check`): prevents infinite types like
`α = List<α>`. traverses all compound type constructors recursively.

**path compression** (`resolve_id`): union-find style. after resolving
`α → β → s32`, updates `α` to point directly to `s32`. tested with
chains up to 20 deep via property tests.

**level-based generalization** (`generalize`): variables created at
levels above `current_level` are quantified. O(1) per variable vs O(n)
environment scanning. `push_scope` increments level, `pop_scope`
decrements.

**undo-log scoping** (`push_scope` / `pop_scope`): O(1) push records a
boundary marker. O(k) pop replays the undo log (k = bindings in that
scope, not total bindings). `bind_var` and `define_ty_alias` record
`Insert` or `Overwrite` entries so pop restores the exact previous
state. no cloning.

**type interning** (`intern_ty`): `HashMap<Ty, TyId>` for O(1) dedup.
inference variables bypass interning (each must be unique).

### test coverage.

  ```
  90 tests total (44 unit + 28 error + 17 property + 1 integration)

  unit tests (src/tests.rs):
    literals        — int/float/bool/str/char inference
    arithmetic      — add/sub/mul/div/rem with type vars
    comparison      — returns bool
    logical/bitwise — bool/int constraints
    unary ops       — neg/not/bitnot
    polymorphism    — let-poly generalize + instantiate
    occurs check    — direct, in-function, nested
    function unify  — param/return unification, arity mismatch
    array/ref       — element unification, nesting, size mismatch
    tuple           — unification, occurs check, generalization, arity
    substitution    — chains, transitivity, path compression
    interning       — type dedup, inference var uniqueness
    scoping         — push/pop, shadowing, restoration, undo log
    annotations     — type annotation + mismatch
    type params     — Param same/different/infer/concrete unification
    alias scoping   — define/shadow/restore through undo log

  error tests (src/tests/errors.rs):
    TypeMismatch         — bool/int, str/bool, char/int, unit/bool,
                           ref mutability, tuple arity, tuple element,
                           Param vs concrete, Param vs different Param,
                           add bool, logical-and int, bitwise bool,
                           concat non-str, neg bool, bitnot bool
    InfiniteType         — direct, in-function, in-ref, in-tuple, nested
    ArgumentCountMismatch — 1 vs 2, 0 vs 1
    ArraySizeMismatch    — fixed sizes, fixed vs dynamic
    UndefinedVariable    — unknown name, after pop_scope
    Error absorption     — Ty::Error unifies without cascading

  property tests (tests/tychecking.rs, proptest):
    reflexivity     — unify(τ, τ) always succeeds
    symmetry        — unify(a,b) ↔ unify(b,a)
    transitivity    — chain unification
    idempotency     — resolve(resolve(x)) = resolve(x)
    determinism     — same ops → same results
    monotonicity    — annotations restrict, not expand
    principal type  — identity fn has ∀α. α → α
    path compress   — chains up to 20 deep
    occurs check    — fun/array/ref self-reference
    completeness    — constraint systems find valid solutions

  fail programs (zo-tests/programming/fail/):
    033-int-concat-str       — int ++ str → TypeMismatch
    034-str-concat-int       — str ++ int → TypeMismatch
    041-arg-count-mismatch   — wrong arity → ArgumentCountMismatch
    044-arg-type-mismatch    — int for str → TypeMismatch
  ```

### benchmarks.

  ```sh
  cargo bench -p zo-ty-checker --bench tycheck --quiet
  ```

  | group | benchmark | metric |
  |-------|-----------|--------|
  | unify | concrete same type | ~3.6 ns/op |
  | unify | infer var → concrete | ~24 ns/op |
  | unify | function types (3 params) | ~350 ns/op |
  | intern | primitive lookup (cached) | ~4 ns/op |
  | intern | fresh_var | ~12 ns/op |
  | scope | push/pop empty | ~5 ns/op |
  | scope | push/bind 10/pop | ~8 ns/cycle |
  | poly | generalize + instantiate | ~183 ns/op |
  | scaling | unifications throughput | **30-42M unif/s** |

### known gaps.

  - [x] `Ty::Param` — rigid type parameter unification (nominal).
  - [x] scope cloning O(n) → undo-log O(1) push / O(k) pop.
  - [x] `tests/common.rs` — shared helpers (assert_unify_error/ok, assert_lookup_error).
  - [x] `tests/errors.rs` — 28 error-path tests covering all 5 ErrorKinds.
  - [x] `benches/tycheck.rs` — criterion benchmarks (unify, intern, scope, poly, scaling).
