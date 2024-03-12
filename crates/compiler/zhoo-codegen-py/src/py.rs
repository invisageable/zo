use zhoo_ast::ast;

pub trait AsPyBuiltin {
  fn as_py_builtin(&self) -> &str;
}

impl AsPyBuiltin for &str {
  fn as_py_builtin(&self) -> &str {
    match *self {
      "println" => "print",
      _ => self,
    }
  }
}

pub trait AsPyOp {
  fn as_py_op(&self) -> &str;
}

impl AsPyOp for ast::BinOp {
  fn as_py_op(&self) -> &str {
    self.kind.as_py_op()
  }
}

impl AsPyOp for ast::BinOpKind {
  fn as_py_op(&self) -> &str {
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

impl AsPyOp for ast::UnOp {
  fn as_py_op(&self) -> &str {
    self.kind.as_py_op()
  }
}

impl AsPyOp for ast::UnOpKind {
  fn as_py_op(&self) -> &str {
    match self {
      Self::Neg => "-",
      Self::Not => "not",
    }
  }
}
