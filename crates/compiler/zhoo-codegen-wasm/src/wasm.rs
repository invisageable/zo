//! ...

use zhoo_ast::ast::{BinOpKind, OutputTy};
use zhoo_ty::ty::{Ty, TyKind};

pub(crate) trait Wat {
  fn as_wat(&self) -> &str;
}

impl Wat for BinOpKind {
  fn as_wat(&self) -> &str {
    match self {
      Self::Add => "add",
      Self::Sub => "sub",
      Self::Mul => "mul",
      Self::Div => "div_s",
      Self::Rem => "rem_s",
      Self::And => "and",
      Self::Or => "or",
      Self::BitAnd => "or",
      Self::BitOr => "or",
      Self::BitXor => "or",
      Self::Eq => "eq",
      Self::Ne => "ne",
      Self::Gt => "gt_s",
      Self::Lt => "lt_s",
      Self::Ge => "ge_s",
      Self::Le => "le_s",
      Self::Shl => "shl",
      Self::Shr => "shr_s",
      _ => todo!(),
    }
  }
}

impl Wat for OutputTy {
  fn as_wat(&self) -> &str {
    match self {
      Self::Default(_) => "i64",
      Self::Ty(ty) => ty.as_wat(),
    }
  }
}

impl Wat for Ty {
  fn as_wat(&self) -> &str {
    self.kind.as_wat()
  }
}

impl Wat for TyKind {
  fn as_wat(&self) -> &str {
    match self {
      Self::Int | Self::Bool | Self::Char | Self::Str => "i64",
      Self::Float => "f64",
      _ => unreachable!(),
    }
  }
}
