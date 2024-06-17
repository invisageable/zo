//! ...

use super::ty::{Ty, TyKind};

impl std::fmt::Display for Ty {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for TyKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Unit => write!(f, "()"),
      Self::Var(var) => write!(f, "{var}"),
    }
  }
}
