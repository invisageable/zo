//! ...

use zo_ast::ast::{BinOp, BinOpKind, OutputTy};

pub(crate) trait AsWat {
  fn as_wat(&self) -> &str;
}

impl AsWat for BinOp {
  fn as_wat(&self) -> &str {
    self.kind.as_wat()
  }
}

impl AsWat for BinOpKind {
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
    }
  }
}

impl AsWat for OutputTy {
  fn as_wat(&self) -> &str {
    match self {
      Self::Default(_) => "i64",
      Self::Ty(_) => todo!(),
    }
  }
}
