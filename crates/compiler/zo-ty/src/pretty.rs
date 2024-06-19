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
      Self::Int => write!(f, "int"),
      Self::Float => write!(f, "float"),
      Self::Ident(ident) => write!(f, "{ident}"),
      Self::Bool => write!(f, "bool"),
      Self::Char => write!(f, "char"),
      Self::Str => write!(f, "str"),
      Self::Var(var) => write!(f, "{var}"),
      _ => todo!(),
    }
  }
}
