//! The pretty print of the `zo` AST module.

use super::ast::{
  Arg, Args, BinOp, BinOpKind, Block, Expr, ExprKind, Ext, Field, Fields, Fun,
  Ident, Input, Inputs, Item, ItemKind, Lit, LitKind, Mutability, OutputTy,
  Pattern, PatternKind, Prototype, Stmt, StmtKind, Struct, StructExpr, TyAlias,
  UnOp, UnOpKind, Var, VarKind,
};

use zo_core::fmt::{sep_comma, sep_newline};

impl std::fmt::Display for Mutability {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "mut"),
      Self::No => write!(f, ""),
    }
  }
}

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

impl std::fmt::Display for Item {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ItemKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Var(var) => write!(f, "{var}"),
      Self::TyAlias(ty_alias) => write!(f, "{ty_alias}"),
      Self::Ext(ext) => write!(f, "{ext}"),
      Self::Struct(structure) => write!(f, "{structure}"),
      Self::Fun(fun) => write!(f, "{fun}"),
    }
  }
}

impl std::fmt::Display for TyAlias {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "type {}", self.pattern)?;

    self
      .maybe_ty
      .as_ref()
      .map(|ty| write!(f, " = {ty};"))
      .unwrap_or(write!(f, ";"))
  }
}

impl std::fmt::Display for Ext {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "ext {}", self.prototype)?;

    self
      .maybe_body
      .as_ref()
      .map(|body| write!(f, " {body}"))
      .unwrap_or(write!(f, ";"))
  }
}

impl std::fmt::Display for Struct {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "struct {} {}", self.ident, self.fields)
  }
}

impl std::fmt::Display for Fields {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_comma(&self.0))
  }
}

impl std::fmt::Display for Field {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}: {}", self.ident, self.ty)
  }
}

impl std::fmt::Display for Fun {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "fun {} {}", self.prototype, self.body)
  }
}

impl std::fmt::Display for Stmt {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for StmtKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Var(var) => write!(f, "{var}"),
      Self::Item(item) => write!(f, "{item}"),
      Self::Expr(expr) => write!(f, "{expr}"),
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
      Self::Fn(prototype, block) => {
        write!(f, "fn {prototype} ")?;

        if block.len() == 1 {
          return write!(f, "-> {}", block[0]);
        }

        write!(f, "{block}")
      }
      Self::Call(callee, args) => write!(f, "{callee}({args})"),
      Self::Array(elmts) => write!(f, "{}", sep_comma(elmts)),
      Self::ArrayAccess(indexed, index) => write!(f, "{indexed}[{index}]"),
      Self::Struct(structure) => write!(f, "{:?}", structure),
      Self::StructAccess(structure, prop) => write!(f, "{structure}.{prop}]"),
      Self::Record(pairs) => write!(f, "{:?}", pairs),
      Self::RecordAccess(record, prop) => write!(f, "{record}.{prop}]"),
      Self::IfElse(condition, consequence, maybe_alternative) => {
        write!(f, "if {condition} {consequence}")?;

        match maybe_alternative {
          Some(alternative) => write!(f, " else {{ {alternative} }}"),
          None => write!(f, ""),
        }
      }
      Self::When(condition, consequence, alternative) => {
        write!(f, "when {condition} ? {consequence} : {alternative};")
      }
      Self::Loop(body) => write!(f, "loop {body}"),
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
      Self::Var(var) => write!(f, "{var}"),
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

    write!(f, "{{\n{}\n}}", sep_newline(&self.stmts))
  }
}

impl std::fmt::Display for Prototype {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(
      f,
      "{} ({}) -> {}",
      self.pattern, self.inputs, self.output_ty
    )
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

impl std::fmt::Display for OutputTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Default(_) => write!(f, "()"),
      Self::Ty(ty) => write!(f, "{ty}"),
    }
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

impl std::fmt::Display for Var {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let kind = &self.kind;
    let pattern = &self.pattern;
    let value = &self.value;

    let ty = self
      .maybe_ty
      .as_ref()
      .map(|ty| format!(": {ty} ="))
      .unwrap_or(":=".to_string());

    write!(f, "{kind} {pattern} {ty} {value};")
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

impl std::fmt::Display for StructExpr {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    todo!()
  }
}

impl std::fmt::Display for Ident {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "${}", self.name)
  }
}
