# zo-constant-folding

Constant expression evaluation for the zo compiler's execution-based compilation model.

- **Constant Folding**: Evaluate compile-time constant expressions
  - Arithmetic operations (add, sub, mul, div, mod)
  - Boolean operations (and, or, not)
  - Comparison operations (==, !=, <, >, <=, >=)
  - Variable tracking for immutable constants
  
## Overview

This module provides compile-time evaluation of constant expressions during HIR execution. It's designed for maximum performance (targeting 5M LoC/s for semantic analysis) while maintaining correctness.

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
- Note: Short-circuiting happens at HIR execution level, not here

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

**Why?** These operations are handled by the executor during HIR execution because:
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
- **Executor**: Orchestrates both during HIR execution
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

The constant folder is used by the executor during HIR execution:

```rust
// In executor.rs
let value = self.constfold.fold_binop(op, &lhs.value, &rhs.value, span);
```

It works alongside constant propagation to optimize code during compilation.
