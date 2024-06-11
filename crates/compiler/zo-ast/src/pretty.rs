//! ...

use super::ast::{
  Arg, Args, BinOp, BinOpKind, Block, Expr, ExprKind, Input, Inputs, Lit,
  LitKind, Pattern, PatternKind, Prototype, UnOp, UnOpKind,
};

use zo_core::fmt::{sep_comma, sep_newline};

impl std::fmt::Display for Pattern {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for PatternKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Underscore => write!(f, "_"),
      Self::Ident(expr) => write!(f, "{expr}"),
      Self::Lit(lit) => write!(f, "{lit}"),
    }
  }
}

impl std::fmt::Display for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ExprKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Lit(lit) => write!(f, "{lit}"),
      Self::UnOp(unop, rhs) => write!(f, "{unop}{rhs}"),
      Self::BinOp(binop, lhs, rhs) => write!(f, "{lhs} {binop} {rhs}"),
      Self::Assign(assignee, value) => write!(f, "{assignee} = {value}"),
      Self::AssignOp(binop, assignee, value) => {
        write!(f, "{assignee} {binop} {value}")
      }
      Self::Block(block) => write!(f, "{block}"),
      Self::Fn(prototype, block) => writeln!(f, "fn {prototype} -> {block}"),
      Self::Call(callee, args) => writeln!(f, "{callee}({args})"),
    }
  }
}

impl std::fmt::Display for Lit {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for LitKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Int(int) => write!(f, "{int}"),
      Self::Float(float) => write!(f, "{float}"),
      Self::Ident(ident) => write!(f, "{ident}"),
      Self::Bool(boolean) => write!(f, "{boolean}"),
      Self::Char(ch) => write!(f, "'{ch}'"),
      Self::Str(string) => write!(f, "\"{string}\""),
    }
  }
}

impl std::fmt::Display for UnOp {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for UnOpKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Neg => write!(f, "-"),
      Self::Not => write!(f, "!"),
    }
  }
}

impl std::fmt::Display for BinOp {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for BinOpKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Add => write!(f, "+"),
      Self::Sub => write!(f, "-"),
      Self::Mul => write!(f, "*"),
      Self::Div => write!(f, "/"),
      Self::Rem => write!(f, "%"),
      Self::And => write!(f, "&&"),
      Self::Or => write!(f, "||"),
      Self::BitAnd => write!(f, "&"),
      Self::BitOr => write!(f, "|"),
      Self::BitXor => write!(f, "^"),
      Self::Lt => write!(f, "<"),
      Self::Gt => write!(f, ">"),
      Self::Le => write!(f, "<="),
      Self::Ge => write!(f, ">="),
      Self::Eq => write!(f, "=="),
      Self::Ne => write!(f, "!="),
      Self::Shl => write!(f, "<<"),
      Self::Shr => write!(f, ">>"),
    }
  }
}

impl std::fmt::Display for Block {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    if self.is_empty() {
      return write!(f, "{{}}");
    }

    write!(f, "{{{}}}", sep_newline(&self.exprs))
  }
}

impl std::fmt::Display for Prototype {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{} ({})", self.pattern, self.inputs)
  }
}

impl std::fmt::Display for Inputs {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_comma(&self.0))
  }
}

impl std::fmt::Display for Input {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.pattern)
  }
}

impl std::fmt::Display for Args {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_comma(&self.0))
  }
}

impl std::fmt::Display for Arg {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.expr)
  }
}
