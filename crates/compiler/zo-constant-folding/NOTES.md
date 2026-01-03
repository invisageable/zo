# Constant Folding

> Related: See `zo-constant-propagation/NOTES.md` for propagating known constant values through variables.
> Constant folding evaluates expressions with constant operands; constant propagation tracks constant values assigned to variables.

| before                  | after               |
| :---------------------- | :------------------ |
| imu x: int = (2+3) * y; | imu x: int = 5 * y; |
| b & false               | false               |

• When is it safely applicable?
  - For Boolean values, yes.
  - For integers, almost always yes.
    - An exception: division by zero.
  - For floating points, use caution.
    - Example: rounding

• General notes about safety:
  - Whether an optimization is safe depends on language semantics.
    - Languages that provide weaker guarantees to the programmer permit more optimizations, but have more ambiguity in their behavior.
  - Is there a formal proof for safety?

## Algebraic Simplification.

• More general form of constant folding
  - Take advantage of mathematically sound simplification rules.

• Identities:
  - a * 1 ➔ a          a * 0 ➔ 0
  - a + 0 ➔ a          a – 0 ➔ a
  - b | false ➔ b      b & true ➔ b

• Reassociation & commutativity:
  - (a + b) + c ➔ a + (b + c)
  - a + b ➔ b + a

• Combined with Constant Folding:
  - (a + 1) + 2 ➔ a + (1 + 2) ➔ a + 3
  - (2 + a) + 4 ➔ (a + 2) + 4 ➔ a + (2 + 4) ➔ a + 6

## Strength Reduction.

• Replace expensive op with cheaper op:
  - a * 4 ➔ a << 2
  - a * 7 ➔ (a << 3) – a
  - a / 32767 ➔ (a >> 15) + (a >> 30)

• So, the effectiveness of this optimization depends on the architecture.

---

Deep Analysis of Constant Folding Optimizations

1. Algebraic Identities - Simplified for Linear Execution

Purpose: Apply algebraic laws to eliminate unnecessary operations during HIR execution.

In zo's execution model, when we reach a binary operator, both operands are already computed and on the stack. We simply check if one operand is a special constant that allows simplification:

```rust
// Identity elements (operation returns unchanged operand)
a + 0 = a     (0 is additive identity)
a * 1 = a     (1 is multiplicative identity)
a | false = a (false is OR identity)
a & true = a  (true is AND identity)
a ^ 0 = a     (0 is XOR identity)

// Absorbing elements (operation returns the absorber)
a * 0 = 0     (0 absorbs multiplication)
a & false = false (false absorbs AND)
a | true = true   (true absorbs OR)

// Idempotent laws (when both operands are the same)
a | a = a
a & a = a
```

Implementation for zo's execution model:

```rust
// FoldResult needs to support passing through existing values
pub enum FoldResult {
  Int(i64),
  Bool(bool),
  Passthrough(ValueId),  // Return existing value unchanged
  Error(Error),
}

impl<'a> ConstFold<'a> {
  fn apply_algebraic_simplification(&self, op: BinOp, lhs: ValueId, rhs: ValueId) -> Option<FoldResult> {
    // Direct pattern matching - no complex tables needed
    match op {
      BinOp::Add => {
        if self.is_const_int(rhs, 0) { return Some(FoldResult::Passthrough(lhs)); }
        if self.is_const_int(lhs, 0) { return Some(FoldResult::Passthrough(rhs)); }
      }
      BinOp::Mul => {
        // Absorber: a * 0 = 0
        if self.is_const_int(rhs, 0) || self.is_const_int(lhs, 0) {
          return Some(FoldResult::Int(0));
        }
        // Identity: a * 1 = a
        if self.is_const_int(rhs, 1) { return Some(FoldResult::Passthrough(lhs)); }
        if self.is_const_int(lhs, 1) { return Some(FoldResult::Passthrough(rhs)); }
      }
      BinOp::Sub => {
        // a - 0 = a
        if self.is_const_int(rhs, 0) { return Some(FoldResult::Passthrough(lhs)); }
        // a - a = 0 (when both operands are identical)
        if lhs == rhs { return Some(FoldResult::Int(0)); }
      }
      BinOp::And => {
        // Absorber: a & false = false
        if self.is_const_bool(rhs, false) || self.is_const_bool(lhs, false) {
          return Some(FoldResult::Bool(false));
        }
        // Identity: a & true = a
        if self.is_const_bool(rhs, true) { return Some(FoldResult::Passthrough(lhs)); }
        if self.is_const_bool(lhs, true) { return Some(FoldResult::Passthrough(rhs)); }
        // Idempotent: a & a = a
        if lhs == rhs { return Some(FoldResult::Passthrough(lhs)); }
      }
      BinOp::Or => {
        // Absorber: a | true = true
        if self.is_const_bool(rhs, true) || self.is_const_bool(lhs, true) {
          return Some(FoldResult::Bool(true));
        }
        // Identity: a | false = a
        if self.is_const_bool(rhs, false) { return Some(FoldResult::Passthrough(lhs)); }
        if self.is_const_bool(lhs, false) { return Some(FoldResult::Passthrough(rhs)); }
        // Idempotent: a | a = a
        if lhs == rhs { return Some(FoldResult::Passthrough(lhs)); }
      }
      // ... other operators
      _ => {}
    }
    None
  }
}
```

2. Strength Reduction - The Cost Model

Purpose: Replace expensive operations with cheaper sequences based on architecture cost models.

From Cooper & Torczon's engineering approach, strength reduction uses machine-dependent cost tables:

```rust
pub struct StrengthPattern {
  // Pattern: mul by constant -> replacement sequence
  multiplier: i64,
  replacement: ReplacementSeq,
  cost_savings: u32,  // cycles saved on target arch
}

pub enum ReplacementSeq {
  Shift(u32),                        // a * 2^n -> a << n
  ShiftAdd(u32),                     // a * 3 -> (a << 1) + a  
  ShiftSub(u32),                     // a * 7 -> (a << 3) - a
  ShiftAddShift(u32, u32),           // a * 5 -> (a << 2) + a
  MagicMul { magic: u64, shift: u32 }, // Division by constant (Granlund-Montgomery)
}

// Algorithm from "Hacker's Delight" by Warren
fn compute_strength_reduction(op: BinOp, constant: i64) -> Option<ReplacementSeq> {
  match op {
    BinOp::Mul => {
      // Power of 2: use shift
      if constant.is_power_of_two() {
        return Some(ReplacementSeq::Shift(constant.trailing_zeros()));
      }
      // Small constants: use shift-add sequences (Bernstein's chains)
      match constant {
        3 => Some(ReplacementSeq::ShiftAdd(1)),     // (a << 1) + a
        5 => Some(ReplacementSeq::ShiftAdd(2)),     // (a << 2) + a  
        7 => Some(ReplacementSeq::ShiftSub(3)),     // (a << 3) - a
        9 => Some(ReplacementSeq::ShiftAdd(3)),     // (a << 3) + a
        // Extended using addition chains algorithm
        _ => compute_addition_chain(constant),
      }
    }
    BinOp::Div if constant > 0 => {
      // Granlund-Montgomery algorithm for division by constant
      let (magic, shift) = compute_magic_multiplier(constant);
      Some(ReplacementSeq::MagicMul { magic, shift })
    }
    _ => None,
  }
}
```

3. Reassociation - Simple in Linear Execution

In zo's postorder execution, reassociation is straightforward because we process expressions bottom-up:

```rust
// Example: (a + 1) + 2
// Execution order:
// 1. Push 'a' onto stack
// 2. Push '1' onto stack  
// 3. Execute '+' -> produces tmp = a + 1
// 4. Push '2' onto stack
// 5. Execute '+' -> sees tmp + 2

// To enable reassociation, track how values were computed:
pub struct ValueProvenance {
  // For each ValueId, track its source
  sources: Vec<ValueSource>,
}

pub enum ValueSource {
  Const(i64),
  Variable(Symbol),
  BinOp { op: BinOp, lhs: ValueId, rhs: ValueId },
}

// CRITICAL: ValueProvenance must be kept synchronized with value creation
// The executor MUST update provenance whenever creating a new value:
impl<'a> Executor<'a> {
  // Helper to ensure provenance is always tracked
  fn store_value_with_source(&mut self, source: ValueSource) -> ValueId {
    let value_id = match &source {
      ValueSource::Const(val) => self.values.store_int(*val),
      ValueSource::Variable(sym) => self.values.store_runtime(*sym),
      ValueSource::BinOp { .. } => self.values.store_runtime(0),
    };
    
    // Critical: Update provenance immediately
    self.provenance.record(value_id, source);
    value_id
  }
  
  // Example usage in binary operation
  fn execute_binary_op(&mut self, op: BinOp, lhs: ValueId, rhs: ValueId) -> ValueId {
    // ... folding attempts ...
    
    // If no folding, create runtime value with provenance
    let source = ValueSource::BinOp { op, lhs, rhs };
    self.store_value_with_source(source)
  }
}

impl<'a> ConstFold<'a> {
  fn reassociate(&self, op: BinOp, lhs: ValueId, rhs: ValueId) -> Option<FoldResult> {
    // Only associative ops can be reassociated
    if !matches!(op, BinOp::Add | BinOp::Mul | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor) {
      return None;
    }

    // Pattern: (a OP c1) OP c2 -> a OP (c1 OP c2)
    // Check if lhs came from same operation with a constant
    if let ValueSource::BinOp { op: inner_op, lhs: a, rhs: c1 } = self.get_source(lhs) {
      if inner_op == op && self.is_const(c1) && self.is_const(rhs) {
        // Fold the two constants together
        if let Some(folded) = self.fold_constants(op, c1, rhs) {
          // Return a OP folded_constant
          // This needs to be handled by executor since we need to emit new SIR
          return Some(FoldResult::Reassociated { base: a, op, constant: folded });
        }
      }
    }

    // Pattern: c1 OP (a OP c2) -> a OP (c1 OP c2) [if commutative]
    if op.is_commutative() {
      if let ValueSource::BinOp { op: inner_op, lhs: a, rhs: c2 } = self.get_source(rhs) {
        if inner_op == op && self.is_const(lhs) && self.is_const(c2) {
          if let Some(folded) = self.fold_constants(op, lhs, c2) {
            return Some(FoldResult::Reassociated { base: a, op, constant: folded });
          }
        }
      }
    }

    None
  }
}
```

## The Complete Algorithm for zo's Execution Model

### All optimizations belong in ConstFold module:

```rust
// Modern compiler approach - unified representation
pub enum FoldResult {
  Scalar(ScalarValue),         // Any scalar constant
  Passthrough(ValueId),        // Identity operation - return existing value
  Reassociated {               // Reassociated expression
    base: ValueId,
    op: BinOp,
    value: ScalarValue,
  },
  StrengthReduced {            // Replaced with cheaper operations
    replacement: Vec<SirInsn>,
  },
  Error(Error),
}

// How modern compilers represent constants efficiently
pub struct ScalarValue {
  data: ScalarData,
  ty: TyId,  // Track the exact type
}

pub enum ScalarData {
  // Store all integers in largest type, track actual width via ty
  Int(i128),   // Can hold any integer value
  Uint(u128),  // Can hold any unsigned value  
  Float(f64),  // f64 can represent f32 exactly
  Bool(bool),
}

// In constfolding.rs - Type-aware optimization
impl<'a> ConstFold<'a> {
  pub fn fold_binop(&self, op: BinOp, lhs: ValueId, rhs: ValueId, ty: TyId, span: Span) -> Option<FoldResult> {
    // 1. Try algebraic simplification (O(1) checks)
    if let Some(result) = self.apply_algebraic_simplification(op, lhs, rhs, ty) {
      return Some(result);
    }

    // 2. Try reassociation for constant folding (if associative)
    if let Some(result) = self.reassociate(op, lhs, rhs, ty) {
      return Some(result);
    }

    // 3. Try constant folding (both operands constant)
    if let Some(result) = self.fold_constants(op, lhs, rhs, ty, span) {
      return Some(result);
    }

    // 4. Try strength reduction (one operand constant)
    if let Some(result) = self.apply_strength_reduction(op, lhs, rhs, ty) {
      return Some(result);
    }

    None
  }

  // Modern approach - single generic implementation
  fn fold_constants(&self, op: BinOp, lhs: ValueId, rhs: ValueId, ty: TyId, span: Span) -> Option<FoldResult> {
    // Get scalar values with their types
    let lhs_scalar = self.get_scalar_value(lhs)?;
    let rhs_scalar = self.get_scalar_value(rhs)?;
    
    // Single implementation that handles all types
    let result = match (lhs_scalar.data, rhs_scalar.data) {
      (ScalarData::Int(l), ScalarData::Int(r)) => {
        self.fold_int_op(op, l, r, ty, span)?
      }
      (ScalarData::Uint(l), ScalarData::Uint(r)) => {
        self.fold_uint_op(op, l, r, ty, span)?
      }
      (ScalarData::Float(l), ScalarData::Float(r)) => {
        self.fold_float_op(op, l, r, ty, span)?
      }
      (ScalarData::Bool(l), ScalarData::Bool(r)) => {
        self.fold_bool_op(op, l, r)?
      }
      _ => return None,
    };
    
    Some(FoldResult::Scalar(result))
  }

  // Single int folding function for all widths
  fn fold_int_op(&self, op: BinOp, lhs: i128, rhs: i128, ty: TyId, span: Span) -> Option<ScalarValue> {
    let ty_kind = self.ty_table.get(ty)?;
    
    // Get bit width and signedness from type
    let (width_bits, signed) = match ty_kind {
      Ty::Int { signed, width } => {
        let bits = match width {
          IntWidth::S8 | IntWidth::U8 => 8,
          IntWidth::S16 | IntWidth::U16 => 16,
          IntWidth::S32 | IntWidth::U32 => 32,
          IntWidth::S64 | IntWidth::U64 => 64,
          IntWidth::Arch => 64, // Assume 64-bit arch
        };
        (bits, signed)
      }
      _ => return None,
    };
    
    // Perform operation in i128, then check overflow
    let result = match op {
      BinOp::Add => {
        let sum = lhs.wrapping_add(rhs);
        self.check_overflow(sum, width_bits, signed, span)?
      }
      BinOp::Sub => {
        let diff = lhs.wrapping_sub(rhs);
        self.check_overflow(diff, width_bits, signed, span)?
      }
      BinOp::Mul => {
        let product = lhs.wrapping_mul(rhs);
        self.check_overflow(product, width_bits, signed, span)?
      }
      BinOp::Div => {
        // Integer division by zero is undefined behavior
        if rhs == 0 {
          return Some(FoldResult::Error(Error::new(ErrorKind::DivisionByZero, span)));
        }
        // Check for overflow: MIN / -1 in signed division
        if signed && lhs == -(1i128 << (width_bits - 1)) && rhs == -1 {
          return Some(FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span)));
        }
        lhs.wrapping_div(rhs)
      }
      BinOp::Rem => {
        // Remainder by zero is undefined behavior
        if rhs == 0 {
          return Some(FoldResult::Error(Error::new(ErrorKind::RemainderByZero, span)));
        }
        // Check for overflow: MIN % -1 in signed remainder
        if signed && lhs == -(1i128 << (width_bits - 1)) && rhs == -1 {
          return Some(FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span)));
        }
        lhs.wrapping_rem(rhs)
      }
      // ... other ops (bitwise, comparisons, etc.)
    };
    
    Some(ScalarValue { 
      data: if signed { ScalarData::Int(result) } else { ScalarData::Uint(result as u128) },
      ty,
    })
  }
  
  // Helper to check overflow for any bit width
  fn check_overflow(&self, value: i128, bits: u32, signed: bool, span: Span) -> Option<i128> {
    if signed {
      let max = (1i128 << (bits - 1)) - 1;  // e.g., 127 for i8
      let min = -(1i128 << (bits - 1));     // e.g., -128 for i8
      if value > max || value < min {
        return Some(FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span)));
      }
    } else {
      let max = (1u128 << bits) - 1;  // e.g., 255 for u8
      if value as u128 > max {
        return Some(FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span)));
      }
    }
    Some(value)
  }
}

// In executor.rs - just calls constfold and handles results
impl<'a> Executor<'a> {
  fn execute_binary_op(&mut self, op: BinOp, node_idx: usize) {
    let rhs = self.value_stack.pop().unwrap();
    let lhs = self.value_stack.pop().unwrap();
    let rhs_ty = self.ty_stack.pop().unwrap();
    let lhs_ty = self.ty_stack.pop().unwrap();
    
    // Get span from the node
    let span = self.tree.spans[node_idx];
    
    // Type checker unifies the types and returns the result type
    let ty_id = self.ty_checker.unify(lhs_ty, rhs_ty, span)?;

    // Single call to constfold for ALL optimizations
    let constfold = ConstFold::new(&self.values);

    if let Some(result) = constfold.fold_binop(op, lhs, rhs, ty_id, span) {
      match result {
        FoldResult::Int(val) => {
          // Push constant and emit ConstInt SIR
          let value_id = self.values.store_int(val);
          self.value_stack.push(value_id);
          self.sir.emit(Insn::ConstInt { value: val, ty_id });
        }
        FoldResult::Bool(val) => {
          // Push constant and emit ConstBool SIR
          let value_id = self.values.store_bool(val);
          self.value_stack.push(value_id);
          self.sir.emit(Insn::ConstBool { value: val, ty_id });
        }
        FoldResult::Passthrough(value_id) => {
          // Identity operation - just push existing value back
          self.value_stack.push(value_id);
          // No new SIR needed - value already computed
        }
        FoldResult::Reassociated { base, op, constant } => {
          // Emit optimized operation with folded constant
          let const_id = self.values.store_int(constant);
          self.sir.emit(Insn::BinOp { op, lhs: base, rhs: const_id, ty_id });
        }
        FoldResult::StrengthReduced { replacement } => {
          // Emit replacement instruction sequence
          for insn in replacement {
            self.sir.emit(insn);
          }
        }
        FoldResult::Error(err) => {
          report_error(err);
        }
      }

      return;
    }

    // Emit normal operation only if no optimization applied
    self.sir.emit(Insn::BinOp { op, lhs, rhs, ty_id });
  }
}
```

## Errors to Add to zo-error/src/error.rs

The following errors need to be added to ErrorKind to handle all constant folding cases:

```rust
// Additional constant folding errors needed:
FloatOverflow,        // When f32/f64 arithmetic produces unexpected infinity
FloatUnderflow,       // When f32/f64 arithmetic produces subnormal/zero
UnsignedUnderflow,    // When unsigned subtraction would wrap (e.g., 0u8 - 1)
```

## Floating-Point Constant Folding Considerations

```rust
// Floating-point folding must handle special values and subnormals
fn fold_f32_op(&self, op: BinOp, lhs: f32, rhs: f32, span: Span) -> Option<FoldResult> {
  match op {
    BinOp::Add => {
      let result = lhs + rhs;
      // Check for overflow to infinity
      if result.is_infinite() && !lhs.is_infinite() && !rhs.is_infinite() {
        return Some(FoldResult::Error(Error::new(ErrorKind::FloatOverflow, span)));
      }
      
      // Check for subnormal results (may differ from runtime on some architectures)
      if result.is_subnormal() {
        // Note: Some architectures flush subnormals to zero (FTZ mode)
        // We follow IEEE 754 default behavior here, but this may need
        // target-specific handling in the future
        return Some(FoldResult::Error(Error::new(ErrorKind::FloatUnderflow, span)));
      }

      Some(FoldResult::Scalar(ScalarValue { 
        data: ScalarData::Float(result as f64), 
        ty: self.ty_table.f32() 
      }))
    }
    BinOp::Div => {
      if rhs == 0.0 {
        // IEEE 754: x/0.0 = ±Inf (not an error!)
        Some(FoldResult::Scalar(ScalarValue {
          data: ScalarData::Float((lhs / rhs) as f64),  // Will be ±Inf or NaN
          ty: self.ty_table.f32()
        }))
      } else {
        let result = lhs / rhs;
        if result.is_subnormal() {
          return Some(FoldResult::Error(Error::new(ErrorKind::FloatUnderflow, span)));
        }
        Some(FoldResult::Scalar(ScalarValue {
          data: ScalarData::Float(result as f64),
          ty: self.ty_table.f32()
        }))
      }
    }
    // Comparisons with NaN always return false (except !=)
    BinOp::Eq => Some(FoldResult::Bool(lhs == rhs)),  // false if either is NaN
    BinOp::Neq => Some(FoldResult::Bool(lhs != rhs)), // true if either is NaN
    _ => // ... other operations
  }
}
```

Note: We assume IEEE 754 default rounding (round-to-nearest, ties-to-even). Subnormal handling follows IEEE 754 but may need adjustment for targets that use flush-to-zero (FTZ) mode.

## Algorithm Design for zo

Unlike traditional compilers that use complex algorithms like Cocke-Schwartz value numbering, zo's execution-based model simplifies optimization:

1. **Algebraic Simplification**: Direct pattern matching on constants - no value numbering needed since values are already computed
2. **Strength Reduction**: Granlund-Montgomery (1994) for division, Bernstein's addition chains for multiplication  
3. **Reassociation**: Simple provenance tracking during linear execution - no tree rebalancing needed

## Why This Works in zo's Model

Your execution-based approach is actually better than traditional AST transformation because:

1. Single pass: All optimizations happen during HIR→SIR execution
2. Natural value flow: Stack-based execution makes value tracking trivial
3. No recursion needed: Postorder execution means children are already processed

The "trickiness" I mentioned was wrong - with your Introducer-like tracking, reassociation is straightforward.

## Performance Analysis of Constant Folding Operations.

### 1. Algebraic Simplification

- __Time Complexity: O(1)__
- Direct comparison checks (is value 0? is value 1?)
- No loops, no recursion, just immediate pattern matching

### 2. Constant Folding (both operands constant)

- __Time Complexity: O(1)__
- Single arithmetic operation (add, mul, div, etc.)
- Overflow checking is also O(1) using CPU flags or bit manipulation

### 3. Strength Reduction

- __Time Complexity: O(1)__
- Pattern matching on constant value (is it power of 2?)
- Table lookup for known patterns (multiplication by 3, 5, 7, etc.)
- Even magic number calculation for division is O(1) with precomputed tables

### 4. Reassociation

- __Time Complexity: O(1)__
- Single lookup in provenance table to check if operand came from previous op
- No tree traversal needed due to linear execution

### 5. Type Resolution

- __Time Complexity: O(1) amortized__
- Hash table lookup in type table
- Path compression in union-find makes subsequent lookups faster

### Overall fold_binop Call:

- __Total: O(1)__
- We check each optimization in sequence, all are O(1)
- Early return on first successful optimization

The key insight: In zo's execution model, everything is already computed when we reach the operator. We're not walking trees (O(n)), we're not doing
recursive substitutions (O(n²)), we're just checking values on the stack. This is why the execution-based model is so efficient.

## Implementation Requirements

To implement all optimizations described in this document:

1. **Already available**: Basic constant folding works with current ValueStorage
2. **Trivial to add**: Algebraic simplification (just pattern matching on constants)  
3. **Needs ValueProvenance tracking**: Reassociation requires tracking how each value was computed
   - **IMPORTANT**: ValueProvenance must be synchronized with all value creation in the executor
   - Use the `store_value_with_source()` helper to ensure provenance is never out of sync
   - Without proper synchronization, reassociation will miss opportunities or produce incorrect results
4. **Architecture-specific**: Strength reduction needs target architecture cost models
