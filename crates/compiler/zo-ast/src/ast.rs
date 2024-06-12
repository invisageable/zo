//! ...

use zo_tokenizer::token::kw::Kw;
use zo_tokenizer::token::op::Op;
use zo_tokenizer::token::{Token, TokenKind};

use zo_core::interner::symbol::Symbol;
use zo_core::span::{AsSpan, Span};

#[derive(Clone, Debug)]
pub enum Pub {
  Yes(Span),
  No,
}

#[derive(Clone, Debug)]
pub enum Mutability {
  Yes(Span),
  No,
}

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
  /// array — `[1, 2, 3, 4]`.
  Array(Vec<Expr>),
  /// array access (index) — `foo[0]`.
  ArrayAccess(Box<Expr>, Box<Expr>),
  /// if else — `if foo == 2 { ... }`.
  IfElse(Box<Expr>, Block, Option<Box<Expr>>),
  /// ternary — `when true ? foo : bar`.
  When(Box<Expr>, Box<Expr>, Box<Expr>),
  /// loop — `loop { ... }`.
  Loop(Block),
  /// while loop — `while foo < 10 { ... }`.
  While(Box<Expr>, Block),
  /// exit return — `return`, `return foo`.
  Return(Option<Box<Expr>>),
  /// exit break — `break`, `break foo`.
  Break(Option<Box<Expr>>),
  /// exit continue — `continue`.
  Continue,
  /// variable — `imu foo := 0`, `mut foo := 0`.
  Var(Var),
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

impl From<&Token> for UnOp {
  fn from(token: &Token) -> Self {
    match token.kind {
      TokenKind::Op(op) => Self {
        kind: UnOpKind::from(op),
        span: token.span,
      },
      _ => unreachable!(),
    }
  }
}

#[derive(Clone, Debug)]
pub enum UnOpKind {
  /// negative — `-`
  Neg,
  /// not — `!`
  Not,
}

impl From<Op> for UnOpKind {
  fn from(op: Op) -> Self {
    match op {
      Op::Minus => Self::Neg,
      Op::Exclamation => Self::Not,
      _ => unreachable!(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct BinOp {
  pub kind: BinOpKind,
  pub span: Span,
}

impl From<&Token> for BinOp {
  fn from(token: &Token) -> Self {
    match token.kind {
      TokenKind::Op(op) => Self {
        kind: BinOpKind::from(op),
        span: token.span,
      },
      _ => unreachable!(),
    }
  }
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

impl From<Op> for BinOpKind {
  fn from(op: Op) -> Self {
    match op {
      Op::Plus => Self::Add,
      Op::Minus => Self::Sub,
      Op::Asterisk => Self::Mul,
      Op::Slash => Self::Div,
      Op::Percent => Self::Rem,
      Op::AmpersandAmpersand => Self::And,
      Op::PipePipe => Self::Or,
      Op::Circumflex => Self::BitXor,
      Op::Ampersand => Self::BitAnd,
      Op::Pipe => Self::BitOr,
      Op::LessThan => Self::Lt,
      Op::GreaterThan => Self::Gt,
      Op::LessThanEqual => Self::Le,
      Op::GreaterThanEqual => Self::Ge,
      Op::EqualEqual => Self::Eq,
      Op::ExclamationEqual => Self::Ne,
      Op::LessThanLessThan => Self::Shl,
      Op::GreaterThanGreaterThan => Self::Shr,
      _ => unreachable!(),
    }
  }
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

#[derive(Clone, Debug)]
pub struct Var {
  pub pubness: Pub,
  pub mutability: Mutability,
  pub kind: VarKind,
  pub pattern: Pattern,
  pub value: Box<Expr>,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum VarKind {
  Imu,
  Mut,
  Val,
}

impl From<Option<&Token>> for VarKind {
  fn from(maybe_token: Option<&Token>) -> Self {
    maybe_token
      .map(|token| match token.kind {
        TokenKind::Kw(Kw::Imu) => VarKind::Imu,
        TokenKind::Kw(Kw::Mut) => VarKind::Mut,
        TokenKind::Kw(Kw::Val) => VarKind::Val,
        _ => unreachable!(),
      })
      .unwrap()
  }
}
