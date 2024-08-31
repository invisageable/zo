use super::ast::{
  Alias, Ast, Async, Attr, AttrKind, BinOp, BinOpKind, Block, Elmt, ElmtKind,
  Expr, ExprKind, Fun, Html, Ident, Input, Item, ItemKind, Lit, LitKind, Name,
  OutputTy, Path, PathSegment, Pattern, PatternKind, Prototype, Pub, Slf, Stmt,
  StmtKind, Text, UnOp, UnOpKind, Var, VarKind, Wasm,
};

use zo_ty::ty::TyKind;

use swisskit::fmt::{sep, sep_comma, sep_newline, sep_space};

use smol_str::ToSmolStr;

impl std::fmt::Display for Pub {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "pub "),
      Self::No => write!(f, ""),
    }
  }
}

impl std::fmt::Display for Async {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "async "),
      Self::No => write!(f, ""),
    }
  }
}

impl std::fmt::Display for Wasm {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Yes(_) => write!(f, "wasm "),
      Self::No => write!(f, ""),
    }
  }
}

impl std::fmt::Display for Pattern {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for PatternKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Underscore => write!(f, "_"),
      Self::Lit(lit) => write!(f, "{lit}"),
      Self::Ident(expr) => write!(f, "{expr}"),
      Self::Path(path) => write!(f, "{path}"),
      Self::Array(patterns) => write!(f, "[{}]", sep_comma(patterns)),
      Self::Tuple(patterns) => write!(f, "({})", sep_comma(patterns)),
      Self::Slf(slf) => write!(f, "{slf}"),
    }
  }
}

impl std::fmt::Display for Slf {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Lower => write!(f, "self"),
      Self::Upper => write!(f, "Self"),
    }
  }
}

impl std::fmt::Display for Path {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{};", sep(&self.segments, "::"))
  }
}

impl std::fmt::Display for PathSegment {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.ident)
  }
}

impl std::fmt::Display for Ast {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", sep_newline(&self.stmts))
  }
}

impl std::fmt::Display for Item {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ItemKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Alias(alias) => write!(f, "{alias}"),
      Self::Var(var) => write!(f, "{var}"),
      Self::Fun(fun) => write!(f, "{fun}"),
      _ => todo!(),
    }
  }
}

impl std::fmt::Display for Alias {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let Self {
      pubness,
      pattern,
      ty,
      ..
    } = self;

    write!(f, "{pubness}type {pattern} = {ty};")
  }
}

impl std::fmt::Display for Var {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Self {
      kind,
      pattern,
      value,
      ..
    } = self;

    let ty = match self.ty.kind {
      TyKind::Infer => ":=".to_smolstr(),
      _ => format!(": {} =", self.ty).to_smolstr(),
    };

    write!(f, "{kind} {pattern} {ty} {value};")
  }
}

impl std::fmt::Display for VarKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VarKind::Imu => write!(f, "imu"),
      VarKind::Mut => write!(f, "mut"),
      VarKind::Val => write!(f, "val"),
    }
  }
}

impl std::fmt::Display for Fun {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "fun {} {}", self.prototype, self.block)
  }
}

impl std::fmt::Display for Stmt {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for StmtKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Var(var) => write!(f, "{var}"),
      Self::Item(item) => write!(f, "{item}"),
      Self::Expr(expr) => write!(f, "{expr}"),
    }
  }
}

impl std::fmt::Display for Expr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for ExprKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Lit(lit) => write!(f, "{lit}"),
      Self::UnOp(unop, rhs) => write!(f, "{unop}{rhs}"),
      Self::BinOp(binop, lhs, rhs) => write!(f, "{lhs} {binop} {rhs}"),
      Self::Assign(assignee, value) => write!(f, "{assignee} = {value}"),
      Self::AssignOp(binop, assignee, value) => {
        write!(f, "{assignee} {binop} {value}")
      }
      Self::Array(elmts) => write!(f, "[{}]", sep_comma(elmts)),
      Self::ArrayAccess(indexed, index) => write!(f, "{indexed}[{index}]"),
      Self::Tuple(elmts) => write!(f, "({})", sep_comma(elmts)),
      Self::TupleAccess(indexed, index) => write!(f, "{indexed}.{index}"),
      Self::Block(block) => write!(f, "{block}"),
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
      Self::Closure(prototype, block) => {
        if let [expr] = block.as_slice() {
          return write!(f, "fn {prototype} -> {expr}");
        }

        write!(f, "fn {prototype} {block}")
      }
      Self::Call(callee, args) => write!(f, "{callee}({})", sep_comma(args)),
      _ => todo!(),
    }
  }
}

impl std::fmt::Display for Lit {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for LitKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Display for Ident {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.sym)
  }
}

impl std::fmt::Display for UnOp {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for UnOpKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Neg => write!(f, "-"),
      Self::Not => write!(f, "!"),
    }
  }
}

impl std::fmt::Display for BinOp {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for BinOpKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if self.is_empty() {
      return write!(f, "{{}}");
    }

    write!(f, "{{\n{}\n}}", sep_newline(&self.stmts))
  }
}

impl std::fmt::Display for Prototype {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Self {
      pattern,
      inputs,
      output_ty,
      ..
    } = self;

    write!(f, "{} ({}): {}", pattern, sep_comma(inputs), output_ty)
  }
}

impl std::fmt::Display for Input {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.pattern)
  }
}

impl std::fmt::Display for OutputTy {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Default(_) => write!(f, "()"),
      Self::Ty(ty) => write!(f, "{ty}"),
    }
  }
}

impl std::fmt::Display for Elmt {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Self {
      kind,
      attrs,
      children,
      ..
    } = self;

    match kind {
      ElmtKind::Unknown => panic!(),
      ElmtKind::Comment(sym) => write!(f, "<!-- {sym} -->"),
      ElmtKind::Name(name) => write!(
        f,
        "<{name} {attrs}>{children}</{name}>",
        attrs = sep_space(attrs),
        children = sep_newline(children),
      ),
      ElmtKind::Tag(tag) => write!(f, "{tag}"),
      ElmtKind::Text(text) => write!(f, "{text}"),
    }
  }
}

impl std::fmt::Display for ElmtKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Unknown => write!(f, ""),
      Self::Comment(sym) => write!(f, "<!-- {sym} -->"),
      Self::Name(name) => write!(f, "{name}"),
      Self::Tag(tag) => write!(f, "{tag}"),
      Self::Text(text) => write!(f, "{text}"),
    }
  }
}

impl std::fmt::Display for Attr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "")
  }
}

impl std::fmt::Display for AttrKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "")
  }
}

impl std::fmt::Display for Text {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.text)
  }
}

impl std::fmt::Display for Name {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Html(html) => write!(f, "{html}"),
      Self::Custom(name) => write!(f, "{name}"),
    }
  }
}

impl std::fmt::Display for Html {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::A => write!(f, "a"),
      Self::Div => write!(f, "div"),
    }
  }
}
