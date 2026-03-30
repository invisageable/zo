# zo-constant-folding

> *...*

## about.

...

---

Constant expression evaluation for the zo compiler's execution-based compilation model.

- **Constant Folding**: Evaluate compile-time constant expressions
  - Arithmetic operations (add, sub, mul, div, mod)
  - Boolean operations (and, or, not)
  - Comparison operations (==, !=, <, >, <=, >=)
  - Variable tracking for immutable constants
  
## Overview

This module provides compile-time evaluation of constant expressions during Tree execution. It's designed for maximum performance (targeting 5M LoC/s for semantic analysis) while maintaining correctness.

## Scope

### What We Fold

**Arithmetic Operations**
- Integer: `+`, `-`, `*`, `/`, `%` with overflow detection
- Float: `+`, `-`, `*`, `/` with NaN/Infinity detection

**Comparison Operations**
- Integer: `<`, `<=`, `>`, `>=`, `==`, `!=`
- Float: `<`, `<=`, `>`, `>=`, `==`, `!=`
- Boolean: `==`, `!=`

**Logical Operations**
- Boolean: `&&`, `||`
- Note: Short-circuiting happens at Tree execution level, not here

**Bitwise Operations**
- Integer: `&`, `|`, `^`, `<<`, `>>`
- Shift operations check for invalid shift amounts

**Unary Operations**
- Negation: `-` for integers and floats
- Logical NOT: `!` for booleans
- Bitwise NOT: `!` for integers (Rust-style, not `~`)

### What We DON'T Fold (By Design)

**Complex Data Structures**
- Array/slice indexing: `arr[0]`
- Tuple access: `tuple.0`
- Struct field access: `struct.field`

**Why?** These operations are handled by the executor during Tree execution because:
1. They require type information that's resolved during execution
2. They may involve memory access patterns beyond simple values
3. The executor already has the context needed for these operations
4. Adding them here would violate "Speed Through Simplicity"

**String/Character Operations**
- String concatenation
- Character comparisons

**Why?** Currently not needed for the compiler's primary use cases. Could be added if metaprogramming requires it.

**Reference Operations**
- Deref, Ref, RefMut operations

**Why?** These are type system operations, not value computations.

## Design Philosophy

This module follows zo's execution-based compilation model where:
- Semantic analysis IS compile-time execution
- Type checking happens DURING SIR building
- Constant folding is just one part of the execution process

The separation of concerns:
- **ConstFolding**: Evaluates constant expressions (this module)
- **ConstProp**: Tracks variable values through control flow
- **Executor**: Orchestrates both during Tree execution
- **Type Checker**: Ensures type correctness during execution

## Error Handling

The module uses zo's continue-on-error approach:
- Errors are reported via `report_error()` to thread-local storage
- Execution continues with `Value::Error` for error recovery
- Multiple errors can be collected in a single pass

Three overflow modes:
1. **Wrap**: Default behavior, wraps on overflow
2. **Track**: Reports errors but continues with wrapped value
3. **Strict**: Reports errors and returns `Value::Error`

## Performance Characteristics

- Zero allocations in hot paths
- All operations are inlined
- Designed for 5M LoC/s semantic analysis throughput
- No string allocations except when tracking overflow (rare)

## Value Semantics

```rust
Value::Unknown  // Not a compile-time constant
Value::Error    // Evaluation failed (division by zero, etc.)
Value::Int(n)   // Constant integer
Value::Float(f) // Constant float
Value::Bool(b)  // Constant boolean
```

## Integration

The constant folder is used by the executor during Tree execution:

```rust
// In executor.rs
let value = self.constfold.fold_binop(op, &lhs.value, &rhs.value, span);
```

It works alongside constant propagation to optimize code during compilation.

## dev.

### overview.

`zo-constant-folding` is a **peephole optimizer** — it evaluates
compile-time constant expressions and rewrites partially-known
expressions using algebraic identities and strength reduction. there
is no single "algorithm" here (unlike HM for type inference). it is a
collection of local rewrite rules, which is the standard approach for
constant folding in production compilers.

the executor creates a `ConstFold` instance at each binop/unop site
and asks "can this be computed at compile time?" the answer is one of:

```
FoldResult::Int(u64)          — fully folded integer
FoldResult::Float(f64)        — fully folded float
FoldResult::Bool(bool)        — fully folded boolean
FoldResult::Forward(Operand)  — algebraic identity (x+0 → x)
FoldResult::Strength(op, val) — cheaper op (x*8 → x<<3)
FoldResult::Error(Error)      — compile-time error (div by zero)
None                          — not foldable (runtime values)
```

### architecture.

```
zo-executor (creates ConstFold per op-site)
  │
  ├── fold_binop(op, lhs, rhs, span, ty) → Option<FoldResult>
  │     ├── both operands known → evaluate directly
  │     ├── one operand known → simplify_binop (algebraic / strength)
  │     └── neither known → None
  │
  └── fold_unop(op, rhs, span, ty) → Option<FoldResult>
        ├── operand known → evaluate directly
        └── operand unknown → None
```

the `ConstFold` borrows a `&ValueStorage` — it reads values by
`ValueId` through SOA arrays (`values.ints`, `values.floats`,
`values.bools`). zero allocations in hot paths.

### data flow.

```
ValueStorage (SOA)
  ├── kinds:   Vec<Value>  ← Int / Float / Bool / Runtime / ...
  ├── indices: Vec<u32>    ← index into typed array
  ├── ints:    Vec<u64>    ← integer values
  ├── floats:  Vec<f64>    ← float values
  └── bools:   Vec<bool>   ← boolean values

ConstFold::int_value(id) → Option<u64>   reads kinds[id] + ints[idx]
ConstFold::float_value(id) → Option<f64> reads kinds[id] + floats[idx]
ConstFold::bool_value(id) → Option<bool> reads kinds[id] + bools[idx]
```

### rewrite rules.

**full evaluation** (both operands are compile-time constants):

| category | operations | width-aware |
|---|---|---|
| int arithmetic | `+` `-` `*` `/` `%` | yes — `checked_*` + `validate_int` per width |
| float arithmetic | `+` `-` `*` `/` | yes — `validate_float` narrows f32 |
| int comparisons | `==` `!=` `<` `<=` `>` `>=` | yes — signed compares as i64 |
| float comparisons | `==` `!=` `<` `<=` `>` `>=` | — |
| bool logic | `&&` `\|\|` `==` `!=` | — |
| int bitwise | `&` `\|` `^` `<<` `>>` | yes — result masked to width |
| unary neg | `-int` `-float` | yes — checked for overflow, f32 narrowed |
| unary not | `!bool` | — |
| unary bitnot | `~int` | yes — masked to width |

**algebraic simplification** (one operand is a known constant):

| rule | int | float | bool |
|---|---|---|---|
| `x + 0 → x` | yes | yes | — |
| `0 + x → x` | yes | yes | — |
| `x - 0 → x` | yes | — | — |
| `x * 1 → x` | yes | yes | — |
| `1 * x → x` | yes | yes | — |
| `x / 1 → x` | yes | yes | — |
| `x * 0 → 0` | yes | **no** (NaN/Inf) | — |
| `0 * x → 0` | yes | **no** (NaN/Inf) | — |
| `x & 0 → 0` | yes | — | — |
| `x \| 0 → x` | yes | — | — |
| `x ^ 0 → x` | yes | — | — |
| `x << 0 → x` | yes | — | — |
| `x >> 0 → x` | yes | — | — |
| `x && true → x` | — | — | yes |
| `x && false → false` | — | — | yes |
| `x \|\| false → x` | — | — | yes |
| `x \|\| true → true` | — | — | yes |

all commutative rules also handle the symmetric case (lhs known).

**strength reduction** (replace expensive op with cheaper one):

| rule | condition | result |
|---|---|---|
| `x * 2^n → x << n` | `n > 0`, any signedness | `Strength(Shl, n)` |
| `x / 2^n → x >> n` | `n > 0`, **unsigned only** | `Strength(Shr, n)` |
| `x % 2^n → x & (2^n-1)` | `n > 0`, **unsigned only** | `Strength(BitAnd, 2^n-1)` |

signed div/rem are NOT reduced — rounding semantics differ for
negatives.

### error detection.

| error | trigger |
|---|---|
| `IntegerOverflow` | result exceeds type's bit width |
| `DivisionByZero` | int or float division by zero |
| `RemainderByZero` | remainder by zero |
| `ShiftAmountTooLarge` | shift amount >= bit width |
| `FloatInfinity` | result overflows f32/f64 to ±infinity |
| `FloatNaN` | result is NaN |

overflow is detected **per width** — `validate_int()` checks against
the actual type (u8, s16, u32, etc.), not just u64/i64.

### test coverage.

```
115 tests total across 6 files

folding.rs (35 tests):
  int arithmetic      — add/sub/mul/div/rem
  int comparisons     — eq/neq/lt/gte
  int bitwise         — and/or/xor/shl/shr, boundary
  float arithmetic    — add/mul/div/lt
  bool logic          — and/or/eq/neq
  unary ops           — neg int/float, not bool, bitnot int
  identity edge cases — add zero, mul one, mul zero
  type mismatch       — int vs float → None
  unsupported ops     — float bitwise, bool add → None

errors.rs (12 tests):
  IntegerOverflow     — add/sub/mul overflow, neg i64::MIN
  DivisionByZero      — int and float
  RemainderByZero     — int
  ShiftAmountTooLarge — shl/shr >= bit width
  runtime fallback    — binop/unop on runtime values → None

simplify.rs (25 tests):
  int identity        — +0, -0, *1, /1, |0, ^0, <<0, >>0
  int absorbing       — *0, &0
  float identity      — +0.0, *1.0, /1.0
  float safety        — *0.0 NOT simplified (NaN/Inf)
  bool identity       — &&true, ||false
  bool absorbing      — &&false, ||true
  both sides          — lhs known, rhs known
  two runtimes        — no simplification → None

strength.rs (14 tests):
  mul→shl             — *2, *8, *1024, signed *4
  div→shr             — unsigned /4
  rem→bitand          — unsigned %8, %256
  signed exclusion    — signed div/rem NOT reduced
  edge cases          — *1 is identity not shift, *3 not reduced

width.rs (21 tests):
  overflow per width  — u8, s8, u16, u32, s32
  signed comparison   — -1 < 0 for s8 (i64 semantics)
  unsigned comparison — same bits, different meaning
  narrow shifts       — u8 shift >= 8 errors, boundary 7 ok
  bitwise masking     — bitnot, bitor, shl masked to width
  signed neg          — s8 negation with validation
  arithmetic shr      — s16 sign-extending right shift
  f32 precision       — narrowing, overflow→infinity, neg validation
  f64 precision       — keeps full precision, overflow→infinity
```

### benchmarks.

```sh
cargo bench -p zo-constant-folding --bench fold --quiet
```

| group | benchmark | metric |
|-------|-----------|--------|
| fold_binop | int add | ~4.6 ns/op |
| fold_binop | int div | ~4.3 ns/op |
| fold_binop | int comparison | ~4.4 ns/op |
| fold_binop | int bitwise | ~4.1 ns/op |
| fold_binop | float add | ~5.4 ns/op |
| fold_binop | bool logic | ~6.8 ns/op |
| simplify | identity x+0 | ~11 ns/op |
| simplify | strength x*8→shl | ~11 ns/op |
| width | s32 add + validation | ~5.4 ns/op |
| scaling | throughput | **~215M folds/s** |

### known gaps.

  - [x] `benches/fold.rs` — criterion benchmarks (binop, simplify, width, scaling).
  - [x] float f32 vs f64 — `validate_float` narrows to f32 precision, detects
        overflow (finite→infinity) and NaN. 6 new tests.
  - [x] `BinOp::Concat` — string constant folding via `FoldResult::Str(Symbol)`.
        interner is mandatory on `ConstFold::new()`. 5 tests.
  - [x] float `%` (fmod) — IEEE 754 remainder with zero-check. 3 new tests.
