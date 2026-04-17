//! # constant folding.
//!
//! > _if operands are known at compile type, perform the operation statically._
//!
//! # example:
//!
//! | before                  | after               |
//! | :---------------------- | :------------------ |
//! | imu x: int = (2+3) * y; | imu x: int = 5 * y; |
//! | b & false               | false               |

use zo_error::{Error, ErrorKind};
use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, UnOp};
use zo_span::Span;
use zo_ty::{FloatWidth, IntWidth, Ty};
use zo_value::{Value, ValueId, ValueStorage};

/// Represents a [`ConstFold`] instance for compile-time evaluation.
pub struct ConstFold<'a> {
  /// The [`ValueStorage`].
  values: &'a ValueStorage,
  /// The string interner (for concat folding).
  interner: &'a mut Interner,
}
impl<'a> ConstFold<'a> {
  /// Creates a new [`ConstFold`] instance.
  pub fn new(values: &'a ValueStorage, interner: &'a mut Interner) -> Self {
    Self { values, interner }
  }

  /// Bit width for an [`IntWidth`].
  const fn bit_width(w: IntWidth) -> u32 {
    match w {
      IntWidth::S8 | IntWidth::U8 => 8,
      IntWidth::S16 | IntWidth::U16 => 16,
      IntWidth::S32 | IntWidth::U32 => 32,
      IntWidth::S64 | IntWidth::U64 | IntWidth::Arch => 64,
    }
  }

  /// Check whether `value` fits in the given integer type.
  /// Returns the (possibly masked) value or an overflow error.
  fn validate_int(
    value: u64,
    signed: bool,
    width: IntWidth,
    span: Span,
  ) -> FoldResult {
    let bits = Self::bit_width(width);

    if bits >= 64 {
      // u64/s64/arch — no masking needed, the raw checked_*
      // ops already handle this.
      return FoldResult::Int(value);
    }

    if signed {
      let as_signed = value as i64;
      let half = 1i64 << (bits - 1);

      if as_signed < -half || as_signed >= half {
        return FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span));
      }

      FoldResult::Int(value)
    } else {
      let max = (1u64 << bits) - 1;

      if value > max {
        return FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span));
      }

      FoldResult::Int(value)
    }
  }

  /// Mask an integer to the given bit width (for bitwise ops).
  const fn mask_to_width(value: u64, width: IntWidth) -> u64 {
    let bits = Self::bit_width(width);

    if bits >= 64 {
      value
    } else {
      value & ((1u64 << bits) - 1)
    }
  }

  /// Mask to `width` bits AND sign-extend the result to 64
  /// bits if `signed` is true and the sign bit is set.
  ///
  /// Why: the codegen's `itoa` path reads the full 64-bit
  /// register and uses `cmp x0, #0 / b.ge` — a 64-bit
  /// signed test — to decide whether to print a leading
  /// `-`. Masking a signed 32-bit result (e.g. `-4` =
  /// `0xFFFFFFFFFFFFFFFC`) to `0x00000000FFFFFFFC` hides
  /// the sign bit in the low 32 and the runtime reads it
  /// as a positive `4294967292`. Keep the masked bit
  /// pattern semantically equal to the signed value by
  /// re-extending the sign.
  const fn mask_and_signext(value: u64, signed: bool, width: IntWidth) -> u64 {
    let bits = Self::bit_width(width);

    if bits >= 64 {
      return value;
    }

    let masked = value & ((1u64 << bits) - 1);

    if !signed {
      return masked;
    }

    let sign_bit = 1u64 << (bits - 1);

    if masked & sign_bit != 0 {
      // Fill bits [bits..64) with 1s.
      masked | (!((1u64 << bits) - 1))
    } else {
      masked
    }
  }

  /// Narrow a float result to the target width.
  ///
  /// For f32: compute as f64 then cast to f32 precision. Reports
  /// `FloatInfinity` if a finite f64 becomes infinite in f32, and
  /// `FloatNaN` if the result is NaN.
  fn validate_float(value: f64, width: FloatWidth, span: Span) -> FoldResult {
    if value.is_nan() {
      return FoldResult::Error(Error::new(ErrorKind::FloatNaN, span));
    }

    match width {
      FloatWidth::F32 => {
        let narrow = value as f32;

        if narrow.is_infinite() && value.is_finite() {
          FoldResult::Error(Error::new(ErrorKind::FloatInfinity, span))
        } else {
          FoldResult::Float(narrow as f64)
        }
      }
      // f64 / arch — no narrowing needed.
      FloatWidth::F64 | FloatWidth::Arch => {
        if value.is_infinite() {
          FoldResult::Error(Error::new(ErrorKind::FloatInfinity, span))
        } else {
          FoldResult::Float(value)
        }
      }
    }
  }

  /// Gets integer value.
  fn int_value(&self, value_id: ValueId) -> Option<u64> {
    let idx = value_id.0 as usize;

    self
      .values
      .kinds
      .get(idx)
      .zip(self.values.indices.get(idx))
      .and_then(|(kind, &raw_idx)| match kind {
        Value::Int => self.values.ints.get(raw_idx as usize).copied(),
        _ => None,
      })
  }

  /// Gets floating-point value.
  fn float_value(&self, value_id: ValueId) -> Option<f64> {
    let idx = value_id.0 as usize;

    self
      .values
      .kinds
      .get(idx)
      .zip(self.values.indices.get(idx))
      .and_then(|(kind, &raw_idx)| match kind {
        Value::Float => self.values.floats.get(raw_idx as usize).copied(),
        _ => None,
      })
  }

  /// Gets boolean value.
  fn bool_value(&self, value_id: ValueId) -> Option<bool> {
    let idx = value_id.0 as usize;

    self
      .values
      .kinds
      .get(idx)
      .zip(self.values.indices.get(idx))
      .and_then(|(kind, &raw_idx)| match kind {
        Value::Bool => self.values.bools.get(raw_idx as usize).copied(),
        _ => None,
      })
  }

  /// Gets string symbol value.
  fn str_value(&self, value_id: ValueId) -> Option<Symbol> {
    let idx = value_id.0 as usize;

    self
      .values
      .kinds
      .get(idx)
      .zip(self.values.indices.get(idx))
      .and_then(|(kind, &raw_idx)| match kind {
        Value::String => self.values.strings.get(raw_idx as usize).copied(),
        _ => None,
      })
  }

  /// Evaluates a binary operation at compile time.
  ///
  /// `ty` is the unified result type from the type checker. When
  /// it is `Ty::Int { signed, width }`, overflow is checked
  /// against the actual bit width instead of raw u64.
  pub fn fold_binop(
    &mut self,
    op: BinOp,
    lhs: ValueId,
    rhs: ValueId,
    span: Span,
    ty: Ty,
  ) -> Option<FoldResult> {
    if let (Some(lhs_val), Some(rhs_val)) =
      (self.int_value(lhs), self.int_value(rhs))
    {
      let (signed, width) = match ty {
        Ty::Int { signed, width } => (signed, width),
        // fallback: treat as u64 when type is infer/unknown.
        _ => (false, IntWidth::U64),
      };

      let bits = Self::bit_width(width) as u64;

      let overflow =
        || FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span));

      match op {
        BinOp::Add => {
          if signed {
            let result = (lhs_val as i64).checked_add(rhs_val as i64);

            Some(match result {
              Some(v) => Self::validate_int(v as u64, true, width, span),
              None => overflow(),
            })
          } else {
            let result = lhs_val.checked_add(rhs_val);

            Some(match result {
              Some(v) => Self::validate_int(v, false, width, span),
              None => overflow(),
            })
          }
        }
        BinOp::Sub => {
          if signed {
            let result = (lhs_val as i64).checked_sub(rhs_val as i64);

            Some(match result {
              Some(v) => Self::validate_int(v as u64, true, width, span),
              None => overflow(),
            })
          } else {
            let result = lhs_val.checked_sub(rhs_val);

            Some(match result {
              Some(v) => Self::validate_int(v, false, width, span),
              None => overflow(),
            })
          }
        }
        BinOp::Mul => {
          if signed {
            let result = (lhs_val as i64).checked_mul(rhs_val as i64);

            Some(match result {
              Some(v) => Self::validate_int(v as u64, true, width, span),
              None => overflow(),
            })
          } else {
            let result = lhs_val.checked_mul(rhs_val);

            Some(match result {
              Some(v) => Self::validate_int(v, false, width, span),
              None => overflow(),
            })
          }
        }
        BinOp::Div => {
          if rhs_val == 0 {
            return Some(FoldResult::Error(Error::new(
              ErrorKind::DivisionByZero,
              span,
            )));
          }

          if signed {
            let result = (lhs_val as i64).checked_div(rhs_val as i64);

            Some(match result {
              Some(v) => Self::validate_int(v as u64, true, width, span),
              None => overflow(), // i64::MIN / -1
            })
          } else {
            Some(Self::validate_int(lhs_val / rhs_val, false, width, span))
          }
        }
        BinOp::Rem => {
          if rhs_val == 0 {
            return Some(FoldResult::Error(Error::new(
              ErrorKind::RemainderByZero,
              span,
            )));
          }

          if signed {
            let result = (lhs_val as i64) % (rhs_val as i64);

            Some(FoldResult::Int(result as u64))
          } else {
            Some(FoldResult::Int(lhs_val % rhs_val))
          }
        }

        // comparisons: signed types compare as i64.
        BinOp::Eq => Some(FoldResult::Bool(lhs_val == rhs_val)),
        BinOp::Neq => Some(FoldResult::Bool(lhs_val != rhs_val)),
        BinOp::Lt => Some(FoldResult::Bool(if signed {
          (lhs_val as i64) < (rhs_val as i64)
        } else {
          lhs_val < rhs_val
        })),
        BinOp::Lte => Some(FoldResult::Bool(if signed {
          (lhs_val as i64) <= (rhs_val as i64)
        } else {
          lhs_val <= rhs_val
        })),
        BinOp::Gt => Some(FoldResult::Bool(if signed {
          (lhs_val as i64) > (rhs_val as i64)
        } else {
          lhs_val > rhs_val
        })),
        BinOp::Gte => Some(FoldResult::Bool(if signed {
          (lhs_val as i64) >= (rhs_val as i64)
        } else {
          lhs_val >= rhs_val
        })),

        // Bitwise & shifts: mask to the declared width,
        // then sign-extend for signed types. See
        // `mask_and_signext` for why — the runtime reads
        // the full 64-bit register via `cmp x0, #0` and
        // needs the sign in the high bits to format
        // signed negatives correctly.
        BinOp::BitAnd => Some(FoldResult::Int(Self::mask_and_signext(
          lhs_val & rhs_val,
          signed,
          width,
        ))),
        BinOp::BitOr => Some(FoldResult::Int(Self::mask_and_signext(
          lhs_val | rhs_val,
          signed,
          width,
        ))),
        BinOp::BitXor => Some(FoldResult::Int(Self::mask_and_signext(
          lhs_val ^ rhs_val,
          signed,
          width,
        ))),
        BinOp::Shl => {
          if rhs_val >= bits {
            Some(FoldResult::Error(Error::new(
              ErrorKind::ShiftAmountTooLarge,
              span,
            )))
          } else {
            Some(FoldResult::Int(Self::mask_and_signext(
              lhs_val << rhs_val,
              signed,
              width,
            )))
          }
        }
        BinOp::Shr => {
          if rhs_val >= bits {
            Some(FoldResult::Error(Error::new(
              ErrorKind::ShiftAmountTooLarge,
              span,
            )))
          } else if signed {
            // Arithmetic shift right for signed types —
            // `i64 >> rhs` already sign-extends
            // correctly, no masking needed (the result
            // can never exceed the declared width's
            // range).
            let result = (lhs_val as i64) >> rhs_val;

            Some(FoldResult::Int(result as u64))
          } else {
            Some(FoldResult::Int(Self::mask_to_width(
              lhs_val >> rhs_val,
              width,
            )))
          }
        }

        _ => None,
      }
    } else if let (Some(lhs_val), Some(rhs_val)) =
      (self.float_value(lhs), self.float_value(rhs))
    {
      let width = match ty {
        Ty::Float(w) => w,
        _ => FloatWidth::F64,
      };

      match op {
        BinOp::Add => {
          Some(Self::validate_float(lhs_val + rhs_val, width, span))
        }
        BinOp::Sub => {
          Some(Self::validate_float(lhs_val - rhs_val, width, span))
        }
        BinOp::Mul => {
          Some(Self::validate_float(lhs_val * rhs_val, width, span))
        }
        BinOp::Div => {
          if rhs_val == 0.0 {
            Some(FoldResult::Error(Error::new(
              ErrorKind::DivisionByZero,
              span,
            )))
          } else {
            Some(Self::validate_float(lhs_val / rhs_val, width, span))
          }
        }
        BinOp::Rem => {
          if rhs_val == 0.0 {
            Some(FoldResult::Error(Error::new(
              ErrorKind::RemainderByZero,
              span,
            )))
          } else {
            Some(Self::validate_float(lhs_val % rhs_val, width, span))
          }
        }

        BinOp::Eq => Some(FoldResult::Bool(lhs_val == rhs_val)),
        BinOp::Neq => Some(FoldResult::Bool(lhs_val != rhs_val)),
        BinOp::Lt => Some(FoldResult::Bool(lhs_val < rhs_val)),
        BinOp::Lte => Some(FoldResult::Bool(lhs_val <= rhs_val)),
        BinOp::Gt => Some(FoldResult::Bool(lhs_val > rhs_val)),
        BinOp::Gte => Some(FoldResult::Bool(lhs_val >= rhs_val)),
        _ => None,
      }
    } else if let (Some(lhs_val), Some(rhs_val)) =
      (self.bool_value(lhs), self.bool_value(rhs))
    {
      match op {
        BinOp::And => Some(FoldResult::Bool(lhs_val && rhs_val)),
        BinOp::Or => Some(FoldResult::Bool(lhs_val || rhs_val)),
        BinOp::Eq => Some(FoldResult::Bool(lhs_val == rhs_val)),
        BinOp::Neq => Some(FoldResult::Bool(lhs_val != rhs_val)),
        _ => None,
      }
    } else if op == BinOp::Concat
      && let (Some(lhs_sym), Some(rhs_sym)) =
        (self.str_value(lhs), self.str_value(rhs))
    {
      let lstr = self.interner.get(lhs_sym);
      let rstr = self.interner.get(rhs_sym);
      let result = format!("{lstr}{rstr}");
      let sym = self.interner.intern(&result);

      Some(FoldResult::Str(sym))
    } else {
      // algebraic simplification — one or both operands may be
      // constant, enabling identity/absorbing rewrites.
      self.simplify_binop(op, lhs, rhs, ty)
    }
  }

  /// Algebraic simplification and strength reduction.
  ///
  /// Handles identity elements (`x + 0 → x`), absorbing
  /// elements (`x * 0 → 0`), and strength reduction
  /// (`x * 2^n → x << n`) when at least one operand is a
  /// compile-time constant.
  ///
  /// Float absorbing ops (`x * 0.0`) are intentionally skipped
  /// because NaN/Inf break them (IEEE 754). Only float identity
  /// ops (`x + 0.0`, `x * 1.0`) are safe.
  fn simplify_binop(
    &self,
    op: BinOp,
    lhs: ValueId,
    rhs: ValueId,
    ty: Ty,
  ) -> Option<FoldResult> {
    let is_unsigned = matches!(ty, Ty::Int { signed: false, .. });
    let lhs_int = self.int_value(lhs);
    let rhs_int = self.int_value(rhs);
    let lhs_float = self.float_value(lhs);
    let rhs_float = self.float_value(rhs);
    let lhs_bool = self.bool_value(lhs);
    let rhs_bool = self.bool_value(rhs);

    // — rhs is a known constant.
    if let Some(r) = rhs_int {
      match (op, r) {
        (BinOp::Add | BinOp::Sub | BinOp::BitOr | BinOp::BitXor, 0)
        | (BinOp::Mul | BinOp::Div, 1)
        | (BinOp::Shl | BinOp::Shr, 0) => {
          return Some(FoldResult::Forward(Operand::Lhs));
        }
        (BinOp::Mul | BinOp::BitAnd, 0) => {
          return Some(FoldResult::Int(0));
        }
        _ => {}
      }

      // strength reduction: x * 2^n → Shl(x, n).
      if op == BinOp::Mul && r.is_power_of_two() && r > 1 {
        let shift = r.trailing_zeros();

        return Some(FoldResult::Strength(BinOp::Shl, shift as u64));
      }

      // strength reduction (unsigned only):
      //   x / 2^n → Shr(x, n)
      //   x % 2^n → BitAnd(x, 2^n - 1)
      if is_unsigned && r.is_power_of_two() && r > 1 {
        if op == BinOp::Div {
          let shift = r.trailing_zeros();

          return Some(FoldResult::Strength(BinOp::Shr, shift as u64));
        }

        if op == BinOp::Rem {
          return Some(FoldResult::Strength(BinOp::BitAnd, r - 1));
        }
      }
    }

    if let Some(r) = rhs_float {
      match op {
        BinOp::Add | BinOp::Sub if r == 0.0 => {
          return Some(FoldResult::Forward(Operand::Lhs));
        }
        BinOp::Mul | BinOp::Div if r == 1.0 => {
          return Some(FoldResult::Forward(Operand::Lhs));
        }
        // note: x * 0.0 is NOT safe (NaN, Inf).
        _ => {}
      }
    }

    if let Some(r) = rhs_bool {
      match (op, r) {
        (BinOp::And, true) | (BinOp::Or, false) => {
          return Some(FoldResult::Forward(Operand::Lhs));
        }
        (BinOp::And, false) => {
          return Some(FoldResult::Bool(false));
        }
        (BinOp::Or, true) => {
          return Some(FoldResult::Bool(true));
        }
        _ => {}
      }
    }

    // — lhs is a known constant (commutative cases).
    if let Some(l) = lhs_int {
      match (op, l) {
        (BinOp::Add | BinOp::BitOr | BinOp::BitXor, 0) | (BinOp::Mul, 1) => {
          return Some(FoldResult::Forward(Operand::Rhs));
        }
        (BinOp::Mul | BinOp::BitAnd, 0) => {
          return Some(FoldResult::Int(0));
        }
        // note: 0 - x and 0 / x are NOT identity ops.
        _ => {}
      }
    }

    if let Some(l) = lhs_float {
      match op {
        BinOp::Add if l == 0.0 => {
          return Some(FoldResult::Forward(Operand::Rhs));
        }
        BinOp::Mul if l == 1.0 => {
          return Some(FoldResult::Forward(Operand::Rhs));
        }
        _ => {}
      }
    }

    if let Some(l) = lhs_bool {
      match (op, l) {
        (BinOp::And, true) | (BinOp::Or, false) => {
          return Some(FoldResult::Forward(Operand::Rhs));
        }
        (BinOp::And, false) => {
          return Some(FoldResult::Bool(false));
        }
        (BinOp::Or, true) => {
          return Some(FoldResult::Bool(true));
        }
        _ => {}
      }
    }

    None
  }

  /// Evaluates a unary operation at compile time.
  pub fn fold_unop(
    &self,
    op: UnOp,
    rhs: ValueId,
    span: Span,
    ty: Ty,
  ) -> Option<FoldResult> {
    let (signed, width) = match ty {
      Ty::Int { signed, width } => (signed, width),
      _ => (false, IntWidth::U64),
    };

    match op {
      UnOp::Neg => {
        if let Some(val) = self.int_value(rhs) {
          // two's complement negation.
          let as_signed = val as i64;

          match as_signed.checked_neg() {
            Some(negated) => {
              Some(Self::validate_int(negated as u64, signed, width, span))
            }
            None => Some(FoldResult::Error(Error::new(
              ErrorKind::IntegerOverflow,
              span,
            ))),
          }
        } else {
          let float_width = match ty {
            Ty::Float(w) => w,
            _ => FloatWidth::F64,
          };

          self
            .float_value(rhs)
            .map(|val| Self::validate_float(-val, float_width, span))
        }
      }
      UnOp::Not => self.bool_value(rhs).map(|val| FoldResult::Bool(!val)),
      UnOp::BitNot => self
        .int_value(rhs)
        .map(|val| FoldResult::Int(Self::mask_to_width(!val, width))),
      _ => None,
    }
  }
}

/// Represents the result of constant folding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FoldResult {
  /// The integer value (stored as unsigned, like literals).
  Int(u64),
  /// The float value.
  Float(f64),
  /// The boolean value.
  Bool(bool),
  /// Forward an existing operand (algebraic identity).
  /// e.g. `x + 0 → Forward(Lhs)`, `0 + x → Forward(Rhs)`.
  Forward(Operand),
  /// Strength reduction: replace the op with a cheaper one.
  /// e.g. `x * 8 → Strength(Shl, 3)` means emit `Shl(lhs, 3)`.
  /// The lhs operand is always forwarded; the `u64` becomes the new rhs
  /// constant.
  Strength(BinOp, u64),
  /// Folded string constant (interned symbol).
  /// e.g. `"hello" ++ "world"` → `Str(sym("helloworld"))`.
  Str(Symbol),
  /// The error.
  Error(Error),
}

/// Which operand to forward in an algebraic simplification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
  Lhs,
  Rhs,
}
