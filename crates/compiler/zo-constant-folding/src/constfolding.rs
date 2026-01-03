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
use zo_sir::{BinOp, UnOp};
use zo_span::Span;
use zo_value::{Value, ValueId, ValueStorage};

/// Represents a [`ConstFold`] instance for compile-time evaluation.
pub struct ConstFold<'a> {
  /// The [`ValueStorage`].
  values: &'a ValueStorage,
}
impl<'a> ConstFold<'a> {
  /// Creates a new [`ConstFold`] instance.
  pub const fn new(values: &'a ValueStorage) -> Self {
    Self { values }
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

  /// Evaluates a binary operation at compile time.
  pub fn fold_binop(
    &self,
    op: BinOp,
    lhs: ValueId,
    rhs: ValueId,
    span: Span,
  ) -> Option<FoldResult> {
    if let (Some(lhs_val), Some(rhs_val)) =
      (self.int_value(lhs), self.int_value(rhs))
    {
      match op {
        BinOp::Add => lhs_val
          .checked_add(rhs_val)
          .map(FoldResult::Int)
          .or_else(|| {
            Some(FoldResult::Error(Error::new(
              ErrorKind::IntegerOverflow,
              span,
            )))
          }),
        BinOp::Sub => lhs_val
          .checked_sub(rhs_val)
          .map(FoldResult::Int)
          .or_else(|| {
            Some(FoldResult::Error(Error::new(
              ErrorKind::IntegerOverflow,
              span,
            )))
          }),
        BinOp::Mul => lhs_val
          .checked_mul(rhs_val)
          .map(FoldResult::Int)
          .or_else(|| {
            Some(FoldResult::Error(Error::new(
              ErrorKind::IntegerOverflow,
              span,
            )))
          }),
        BinOp::Div => {
          if rhs_val == 0 {
            Some(FoldResult::Error(Error::new(
              ErrorKind::DivisionByZero,
              span,
            )))
          } else {
            Some(FoldResult::Int(lhs_val / rhs_val))
          }
        }
        BinOp::Rem => {
          if rhs_val == 0 {
            Some(FoldResult::Error(Error::new(
              ErrorKind::RemainderByZero,
              span,
            )))
          } else {
            Some(FoldResult::Int(lhs_val % rhs_val))
          }
        }

        BinOp::Eq => Some(FoldResult::Bool(lhs_val == rhs_val)),
        BinOp::Neq => Some(FoldResult::Bool(lhs_val != rhs_val)),
        BinOp::Lt => Some(FoldResult::Bool(lhs_val < rhs_val)),
        BinOp::Lte => Some(FoldResult::Bool(lhs_val <= rhs_val)),
        BinOp::Gt => Some(FoldResult::Bool(lhs_val > rhs_val)),
        BinOp::Gte => Some(FoldResult::Bool(lhs_val >= rhs_val)),

        BinOp::BitAnd => Some(FoldResult::Int(lhs_val & rhs_val)),
        BinOp::BitOr => Some(FoldResult::Int(lhs_val | rhs_val)),
        BinOp::BitXor => Some(FoldResult::Int(lhs_val ^ rhs_val)),
        BinOp::Shl => Some(FoldResult::Int(lhs_val << rhs_val)),
        BinOp::Shr => Some(FoldResult::Int(lhs_val >> rhs_val)),

        _ => None,
      }
    } else if let (Some(lhs_val), Some(rhs_val)) =
      (self.float_value(lhs), self.float_value(rhs))
    {
      match op {
        BinOp::Add => Some(FoldResult::Float(lhs_val + rhs_val)),
        BinOp::Sub => Some(FoldResult::Float(lhs_val - rhs_val)),
        BinOp::Mul => Some(FoldResult::Float(lhs_val * rhs_val)),
        BinOp::Div => Some(FoldResult::Float(lhs_val / rhs_val)),

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
    } else {
      None
    }
  }

  /// Evaluates a unary operation at compile time.
  pub fn fold_unop(
    &self,
    op: UnOp,
    rhs: ValueId,
    span: Span,
  ) -> Option<FoldResult> {
    match op {
      UnOp::Neg => self.int_value(rhs).map(|val| {
        if val == 0 {
          FoldResult::Int(0)
        } else {
          FoldResult::Error(Error::new(ErrorKind::IntegerOverflow, span))
        }
      }),
      UnOp::Not => self.bool_value(rhs).map(|val| FoldResult::Bool(!val)),
      UnOp::BitNot => self.int_value(rhs).map(|val| FoldResult::Int(!val)),
      _ => None,
    }
  }
}

/// Represents the result of constant folding.
#[derive(Debug, Clone, Copy)]
pub enum FoldResult {
  /// The integer value (stored as unsigned, like literals).
  Int(u64),
  /// The float value.
  Float(f64),
  /// The boolean value.
  Bool(bool),
  /// The error.
  Error(Error),
}
