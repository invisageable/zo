//! ...

use zo_ast::ast::{BinOp, BinOpKind, UnOp, UnOpKind};

pub trait AsPy {
  fn as_py(&self) -> &str;
}

impl AsPy for BinOp {
  fn as_py(&self) -> &str {
    self.kind.as_py()
  }
}

impl AsPy for BinOpKind {
  fn as_py(&self) -> &str {
    match self {
      Self::Add => "+",
      Self::Sub => "-",
      Self::Mul => "*",
      Self::Div => "/",
      Self::Rem => "%",
      Self::And => "and",
      Self::Or => "or",
      Self::BitAnd => "&",
      Self::BitOr => "|",
      Self::BitXor => "^",
      Self::Lt => "<",
      Self::Gt => ">",
      Self::Le => "<=",
      Self::Ge => ">=",
      Self::Eq => "==",
      Self::Ne => "!=",
      Self::Shl => "<<",
      Self::Shr => ">>",
    }
  }
}

impl AsPy for UnOp {
  fn as_py(&self) -> &str {
    self.kind.as_py()
  }
}

impl AsPy for UnOpKind {
  fn as_py(&self) -> &str {
    match self {
      Self::Neg => "-",
      Self::Not => "not",
    }
  }
}
