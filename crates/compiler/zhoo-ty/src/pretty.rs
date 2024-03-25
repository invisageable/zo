mod tast;
mod thir;

use super::ty::{Ty, TyKind};

use zo_core::fmt::sep_comma;

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
      Self::Ident(symbol) => write!(f, "{symbol}"),
      Self::Bool => write!(f, "bool"),
      Self::Char => write!(f, "char"),
      Self::Str => write!(f, "str"),
      Self::Fun((ty, tys)) => write!(f, "{ty}-{}", sep_comma(tys)),
      Self::Infer => write!(f, "_"),
      Self::Alias(symbol) => write!(f, "{symbol}"),
      Self::Pointer(ty) => write!(f, "{ty}"),
      Self::Array(ty) => write!(f, "{ty}"),
      Self::StructExpr(symbol) => write!(f, "{symbol}"),
    }
  }
}
