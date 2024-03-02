//! ...

use zhoo_ty::ty::{Ty, TyKind};

pub(crate) trait Wat {
  fn as_wat(&self) -> &str;
}

impl Wat for Ty {
  fn as_wat(&self) -> &str {
    self.kind.as_wat()
  }
}

impl Wat for TyKind {
  fn as_wat(&self) -> &str {
    match self {
      Self::Bool | Self::Int | Self::Char | Self::Str => "i64",
      Self::Float => "f64",
      _ => unreachable!(),
    }
  }
}
