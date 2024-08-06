use super::ast::{
  Ast, BinOp, BinOpKind, Block, Expr, ExprKind, Item, ItemKind, Lit, LitKind,
  Mutability, Pattern, PatternKind, Stmt, StmtKind, UnOp, UnOpKind, Var,
  VarKind,
};

use zo_ty::ty::TyKind;

use swisskit::fmt::{sep_comma, sep_newline};

use smol_str::ToSmolStr;

impl std::fmt::Display for Mutability {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, ""),
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

impl std::fmt::Display for Ast {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", sep_newline(&self.stmts))
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
    }
  }
}

impl std::fmt::Display for Var {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let kind = &self.kind;
    let pattern = &self.pattern;
    let value = &self.value;

    let ty = match self.ty.kind {
      TyKind::Infer => ":=".to_smolstr(),
      _ => format!(": {} =", self.ty).to_smolstr(),
    };

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
      Self::Array(elmts) => write!(f, "{}", sep_comma(elmts)),
      Self::ArrayAccess(indexed, index) => write!(f, "{indexed}[{index}]"),
      Self::IfElse(condition, consequence, maybe_alternative) => {
        write!(f, "if {condition} {consequence}")?;

        match maybe_alternative {
          Some(alternative) => write!(f, " else {{ {alternative} }}"),
          None => Ok(()),
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
      Self::Int(sym, _) => write!(f, "{sym}"),
      Self::Float(sym) => write!(f, "{sym}"),
      Self::Ident(sym) => write!(f, "{sym}"),
      Self::Bool(sym) => write!(f, "{sym}"),
      Self::Char(sym) => write!(f, "'{sym}'"),
      Self::Str(sym) => write!(f, "\"{sym}\""),
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
