use zhoo_ast::ast;

pub trait AsBuiltin {
  fn as_builtin(&self) -> &str;
}

impl AsBuiltin for &str {
  fn as_builtin(&self) -> &str {
    match *self {
      "println" => "print",
      _ => self,
    }
  }
}

pub trait AsOp {
  fn as_op(&self) -> &str;
}

impl AsOp for ast::BinOp {
  fn as_op(&self) -> &str {
    self.kind.as_op()
  }
}

impl AsOp for ast::BinOpKind {
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
      _ => todo!(),
    }
  }
}

impl AsOp for ast::UnOp {
  fn as_op(&self) -> &str {
    self.kind.as_op()
  }
}

impl AsOp for ast::UnOpKind {
  fn as_op(&self) -> &str {
    match self {
      Self::Neg => "-",
      Self::Not => "not",
    }
  }
}
