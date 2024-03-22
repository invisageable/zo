use zo_core::fmt::{sep_comma, sep_newline};

use super::hir::{
  Arg, Args, Async, BinOp, BinOpKind, Block, Expr, ExprKind, Fun, Hir, Input,
  Inputs, Item, ItemKind, Lit, LitKind, Mutability, OutputTy, Pattern,
  PatternKind, Prototype, Pub, Stmt, StmtKind, TyAlias, UnOp, UnOpKind, Var,
  VarKind, Wasm,
};

impl std::fmt::Display for Pub {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Pub::Yes(_) => write!(f, "pub"),
      Pub::No => write!(f, " "),
    }
  }
}

impl std::fmt::Display for Async {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "async"),
      Self::No => write!(f, ""),
    }
  }
}

impl std::fmt::Display for Wasm {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "wasm"),
      Self::No => write!(f, ""),
    }
  }
}

impl std::fmt::Display for Mutability {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "mut"),
      Self::No => write!(f, ""),
    }
  }
}

impl std::fmt::Display for Pattern<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for PatternKind<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Underscore => write!(f, "_"),
      Self::Ident(symbol) => write!(f, "{symbol}"),
      Self::Lit(lit) => write!(f, "{lit}"),
      Self::MeLower => write!(f, "me"),
    }
  }
}

impl std::fmt::Display for Hir<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_newline(self.items))
  }
}

impl std::fmt::Display for Item<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ItemKind<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::TyAlias(ty_alias) => write!(f, "{ty_alias}"),
      Self::Var(var) => write!(f, "{var}"),
      Self::Fun(fun) => write!(f, "{fun}"),
      _ => todo!(),
    }
  }
}

impl std::fmt::Display for TyAlias<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "")
  }
}

impl std::fmt::Display for Var<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "")
  }
}

impl std::fmt::Display for VarKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      VarKind::Imu => write!(f, "imu"),
      VarKind::Mut => write!(f, "mut"),
      VarKind::Val => write!(f, "val"),
    }
  }
}

impl std::fmt::Display for Fun<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "")
  }
}

impl std::fmt::Display for Prototype<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{} ({})", self.pattern, self.inputs)
  }
}

impl std::fmt::Display for Inputs<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_comma(self.0))
  }
}

impl std::fmt::Display for Input<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "")
  }
}

impl std::fmt::Display for OutputTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Default(_) => write!(f, ""),
    }
  }
}

impl std::fmt::Display for Block<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{{\n{}\n}}", sep_newline(self.stmts))
  }
}

impl std::fmt::Display for Stmt<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for StmtKind<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Item(item) => write!(f, "{item}"),
      Self::Expr(expr) => write!(f, "{expr}"),
    }
  }
}

impl std::fmt::Display for Expr<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ExprKind<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Lit(lit) => write!(f, "{lit}"),
      Self::UnOp(unop, rhs) => write!(f, "{unop}{rhs}"),
      Self::BinOp(binop, lhs, rhs) => write!(f, "{lhs} {binop} {rhs}"),
      Self::Array(elements) => write!(f, "{}", sep_comma(elements)),
      Self::Tuple(elements) => write!(f, "{}", sep_comma(elements)),
      Self::ArrayAccess(array, access) => write!(f, "{array}[{access}]"),
      Self::TupleAccess(_tuple, _access) => write!(f, ""),
      Self::Block(block) => write!(f, "{block}"),
      Self::Fn(prototype, body) => write!(f, "fn {prototype} {body}"),
      Self::Call(callee, args) => write!(f, "{callee}({args})"),
      Self::IfElse(condition, consequence, maybe_alternative) => {
        write!(f, "if {condition} {{\n{consequence}\n}}")?;

        match maybe_alternative {
          Some(alternative) => write!(f, " else {alternative}"),
          None => write!(f, ""),
        }
      }
      Self::When(condition, consequence, alternative) => {
        write!(f, "when {condition} ? {consequence} : {alternative}")
      }
      Self::Loop(body) => write!(f, "{body}"),
      Self::While(condition, body) => write!(f, "while {condition} {body}"),
      Self::Return(maybe_expr) => match maybe_expr {
        Some(expr) => write!(f, "return {expr};"),
        None => write!(f, "return;"),
      },
      Self::Break(maybe_expr) => match maybe_expr {
        Some(expr) => write!(f, "break {expr};"),
        None => write!(f, "break;"),
      },
      Self::Continue => write!(f, "continue"),
      Self::Chaining(lhs, rhs) => write!(f, "{lhs}.{rhs}"),
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
      Self::Int(symbol) => write!(f, "{symbol}"),
      Self::Float(symbol) => write!(f, "{symbol}"),
      Self::Ident(symbol) => write!(f, "{symbol}"),
      Self::Bool(boolean) => write!(f, "{boolean}"),
      Self::Char(symbol) => write!(f, "{symbol}"),
      Self::Str(symbol) => write!(f, "{symbol}"),
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
      Self::Rem => write!(f, "^"),
      Self::And => write!(f, "&&"),
      Self::Or => write!(f, "||"),
      Self::BitXor => write!(f, "^"),
      Self::BitAnd => write!(f, "&"),
      Self::BitOr => write!(f, "|"),
      Self::Lt => write!(f, "<"),
      Self::Gt => write!(f, ">"),
      Self::Le => write!(f, "<="),
      Self::Ge => write!(f, ">="),
      Self::Eq => write!(f, "=="),
      Self::Ne => write!(f, "!="),
      Self::Shl => write!(f, "<<"),
      Self::Shr => write!(f, ">>"),
      Self::Range => write!(f, ".."),
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

impl std::fmt::Display for Args<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "")
  }
}

impl std::fmt::Display for Arg<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.pattern)
  }
}
