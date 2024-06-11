//! ...

use zo_core::interner::symbol::Symbol;
use zo_core::span::{AsSpan, Span};

#[derive(Clone, Debug)]
pub struct Pattern {
  pub kind: PatternKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum PatternKind {
  /// underscore — `_`.
  Underscore,
  /// identifier — `foo`, `bar`.
  Ident(Box<Expr>),
  /// literals.
  Lit(Lit),
}

#[derive(Clone, Debug, Default)]
pub struct Ast {
  pub exprs: Vec<Expr>,
}

impl Ast {
  /// no allocations.
  #[inline]
  pub fn new() -> Self {
    Self {
      exprs: Vec::with_capacity(0usize),
    }
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.exprs.is_empty()
  }

  #[inline]
  pub fn add_expr(&mut self, expr: Expr) {
    self.exprs.push(expr);
  }
}

impl AsSpan for Ast {
  fn as_span(&self) -> Span {
    let lo = self.exprs.first();
    let hi = self.exprs.last();

    match (lo, hi) {
      (Some(first), Some(last)) => Span::merge(first.span, last.span),
      (Some(first), None) => first.span,
      _ => Span::ZERO,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Expr {
  pub kind: ExprKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
  /// literal — `1`, `1.5`, `foobar`, etc.
  Lit(Lit),
  /// prefix — `-1`, `!true`.
  UnOp(UnOp, Box<Expr>),
  /// infix — `1 + 2`, `3 - 4`.
  BinOp(BinOp, Box<Expr>, Box<Expr>),
  /// assignment — `foo = bar`.
  Assign(Box<Expr>, Box<Expr>),
  /// assignment operator — `foo += bar`.
  AssignOp(BinOp, Box<Expr>, Box<Expr>),
  /// block — `{ ... }`.
  Block(Block),
  /// closure — `fn() -> ...`, `fn() { ... }`.
  Fn(Prototype, Block),
  /// call function — `foo()`, `bar(1, 2)`.
  Call(Box<Expr>, Args),
}

#[derive(Clone, Debug)]
pub struct Lit {
  pub kind: LitKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum LitKind {
  Int(Symbol),
  Float(Symbol),
  Ident(Symbol),
  Bool(bool),
  Char(Symbol),
  Str(Symbol),
}

#[derive(Clone, Debug)]
pub struct UnOp {
  pub kind: UnOpKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum UnOpKind {
  /// negative — `-`
  Neg,
  /// not — `!`
  Not,
}

#[derive(Clone, Debug)]
pub struct BinOp {
  pub kind: BinOpKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum BinOpKind {
  /// addition — `+`
  Add,
  /// subtraction — `-`
  Sub,
  /// multiplication — `*`
  Mul,
  /// division — `/`
  Div,
  /// modulus — `%`
  Rem,
  /// logical and — `&&`
  And,
  /// logical or — `||`
  Or,
  /// bitwise and — `&`
  BitAnd,
  /// bitwise or — `|`
  BitOr,
  /// bitwise xor — `^`
  BitXor,
  /// less than — `<`
  Lt,
  /// greater than — `>`
  Gt,
  /// less than or equal — `<=`
  Le,
  /// greater than or equal — `>=`
  Ge,
  /// equality — `==`
  Eq,
  /// not equal — `!=`
  Ne,
  /// shift left — `<<`
  Shl,
  /// shift right — `>>`
  Shr,
}

#[derive(Clone, Debug)]
pub struct Block {
  pub exprs: Vec<Expr>,
}

impl Block {
  /// no allocations.
  #[inline]
  pub fn new() -> Self {
    Self {
      exprs: Vec::with_capacity(0usize),
    }
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.exprs.is_empty()
  }

  #[inline]
  pub fn add_expr(&mut self, expr: Expr) {
    self.exprs.push(expr)
  }
}

impl AsSpan for Block {
  fn as_span(&self) -> Span {
    let lo = self.exprs.first();
    let hi = self.exprs.last();

    match (lo, hi) {
      (Some(first), Some(last)) => Span::merge(first.span, last.span),
      (Some(first), None) => first.span,
      _ => Span::ZERO,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Prototype {
  pub pattern: Pattern,
  pub inputs: Inputs,
  pub output_ty: OutputTy,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Inputs(pub Vec<Input>);

impl Inputs {
  #[inline]
  pub fn new() -> Self {
    Self(Vec::with_capacity(0usize))
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  #[inline]
  pub fn add_input(&mut self, input: Input) {
    self.0.push(input)
  }
}

impl AsSpan for Inputs {
  fn as_span(&self) -> Span {
    let lo = self.first();
    let hi = self.last();

    match (lo, hi) {
      (Some(first), Some(last)) => Span::merge(first.span, last.span),
      (Some(first), None) => first.span,
      _ => Span::ZERO,
    }
  }
}

impl std::ops::Deref for Inputs {
  type Target = Vec<Input>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
pub struct Input {
  pub pattern: Pattern,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum OutputTy {
  Default(Span),
  // Ty(Ty),
}

#[derive(Clone, Debug)]
pub struct Args(pub Vec<Arg>);

impl AsSpan for Args {
  fn as_span(&self) -> Span {
    let lo = self.first();
    let hi = self.last();

    match (lo, hi) {
      (Some(first), Some(last)) => Span::merge(first.span, last.span),
      (Some(first), None) => first.span,
      _ => Span::ZERO,
    }
  }
}

impl Args {
  #[inline]
  pub fn new() -> Self {
    Self(Vec::with_capacity(0usize))
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  #[inline]
  pub fn add_arg(&mut self, arg: Arg) {
    self.0.push(arg)
  }
}

impl std::ops::Deref for Args {
  type Target = Vec<Arg>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
pub struct Arg {
  pub expr: Expr,
  pub span: Span,
}
