use super::value::{Value, ValueKind};

impl std::fmt::Display for Value {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ValueKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Unit => write!(f, "()"),
      Self::Int(int) => write!(f, "{int}"),
      Self::Float(float) => write!(f, "{float}"),
      Self::Ident(ident) => write!(f, "{ident}"),
      Self::Bool(bool) => write!(f, "{bool}"),
      Self::Char(bool) => write!(f, "{bool}"),
      Self::Str(bool) => write!(f, "{bool}"),
      Self::UnOp(unop, lhs) => write!(f, "{unop} {lhs}"),
      Self::BinOp(binop, lhs, rhs) => write!(f, "{lhs} {binop} {rhs}"),
    }
  }
}
