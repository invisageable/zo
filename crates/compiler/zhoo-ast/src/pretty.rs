//! ...wip.

use super::ast::{
  Abstract, Apply, Arg, Args, Async, BinOp, BinOpKind, Block, Expr, ExprKind,
  Ext, Fun, Input, Inputs, Item, ItemKind, Lit, LitKind, Mutability, OutputTy,
  Pattern, PatternKind, Program, Prototype, Pub, Stmt, StmtKind, Struct,
  StructExpr, TyAlias, UnOp, UnOpKind, Var, VarKind, Wasm,
};

use zo_core::fmt::{sep_comma, sep_newline};

impl std::fmt::Display for Pub {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "pub"),
      Self::No => write!(f, ""),
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
      Self::MeLower => write!(f, "me"),
    }
  }
}

impl std::fmt::Display for Program {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_newline(&self.items))
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
      Self::Abstract(abstr) => write!(f, "{abstr}"),
      Self::Fun(fun) => write!(f, "{fun}"),
      _ => todo!(),
    }
  }
}

impl std::fmt::Display for Var {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let kind = &self.kind;
    let pattern = &self.pattern;

    let ty = self
      .maybe_ty
      .as_ref()
      .map_or(":=".to_string(), |ty| format!(": {ty} = "));

    let value = &self.value;

    write!(f, "{kind} {pattern} {ty} {value};",)
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

impl std::fmt::Display for TyAlias {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

impl std::fmt::Display for Ext {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{} ext {}", self.pubness, self.prototype)?;

    let Some(body) = &self.maybe_body else {
      return write!(f, ";");
    };

    write!(f, " {body}")
  }
}

impl std::fmt::Display for Abstract {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

impl std::fmt::Display for Struct {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

impl std::fmt::Display for Apply {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

impl std::fmt::Display for Fun {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "fun {} {}", self.prototype, self.body)
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
    write!(f, "{}: {}", self.pattern, self.ty)
  }
}

impl std::fmt::Display for OutputTy {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      OutputTy::Default(_) => write!(f, ""),
      OutputTy::Ty(ty) => write!(f, ": {ty}"),
    }
  }
}

impl std::fmt::Display for Block {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{{\n{}\n}}", sep_newline(&self.stmts))
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
      Self::UnOp(op, rhs) => write!(f, "({op} {rhs})"),
      Self::BinOp(lhs, op, rhs) => write!(f, "({lhs} {op} {rhs})"),
      Self::Assign(lhs, rhs) => write!(f, "{lhs} = {rhs}"),
      Self::AssignOp(lhs, op, rhs) => write!(f, "{lhs} {op} {rhs}"),
      Self::Array(element) => write!(f, "[{}]", sep_comma(element)),
      Self::Tuple(element) => write!(f, "({})", sep_comma(element)),
      Self::ArrayAccess(indexed, index) => write!(f, "{}[{}]", indexed, index),
      Self::TupleAccess(tuple, access) => write!(f, "{tuple}.{access}"),
      Self::Block(body) => write!(f, "{body}"),
      Self::Fn(prototype, body) => write!(f, "fn {prototype} {body}"),
      Self::Call(callee, args) => write!(f, "{callee}({})", args),
      Self::Return(maybe_expr) => match maybe_expr {
        Some(expr) => write!(f, "return {expr};"),
        None => write!(f, "return;"),
      },
      Self::IfElse(condition, consequence, maybe_alternative) => {
        write!(f, "if {condition} {consequence}")?;

        match maybe_alternative {
          Some(alternative) => write!(f, " {alternative}"),
          None => write!(f, " "),
        }
      }
      Self::When(condition, consequence, alternative) => {
        write!(f, "when {condition} ? {consequence} : {alternative};")
      }
      Self::Loop(body) => write!(f, "for {body}"),
      Self::While(condition, body) => write!(f, "while {condition} {body}"),
      Self::Break(maybe_expr) => match maybe_expr {
        Some(expr) => write!(f, "break {expr};"),
        None => write!(f, "break;"),
      },
      Self::Continue => write!(f, "continue"),
      Self::Var(var) => write!(f, "{var}"),
      Self::StructExpr(struct_expr) => write!(f, "{struct_expr}"),
      Self::Chaining(lhs, rhs) => write!(f, "{lhs}.{rhs}"),
      _ => panic!(),
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

impl std::fmt::Display for Args {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_comma(&self.0))
  }
}

impl std::fmt::Display for Arg {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}: {}", self.pattern, self.ty)
  }
}

impl std::fmt::Display for StructExpr {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{:?}", self.props)
  }
}
