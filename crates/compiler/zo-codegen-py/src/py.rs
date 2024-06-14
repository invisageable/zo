//! ...

use zo_ast::ast::{BinOp, BinOpKind, UnOp, UnOpKind};

pub trait AsOp {
  fn as_op(&self) -> &str;
}

impl AsOp for BinOp {
  fn as_op(&self) -> &str {
    self.kind.as_op()
  }
}

impl AsOp for BinOpKind {
  fn as_op(&self) -> &str {
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

impl AsOp for UnOp {
  fn as_op(&self) -> &str {
    self.kind.as_op()
  }
}

impl AsOp for UnOpKind {
  fn as_op(&self) -> &str {
    match self {
      Self::Neg => "-",
      Self::Not => "not",
    }
  }
}
