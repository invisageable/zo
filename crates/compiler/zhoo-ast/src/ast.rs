//! ...wip.

use zhoo_tokenizer::token::kw::Kw;
use zhoo_tokenizer::token::op::Op;
use zhoo_tokenizer::token::{Token, TokenKind};
use zhoo_ty::ty::Ty;

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::span::{AsSpan, Span};

#[derive(Clone, Debug)]
pub enum Pub {
  Yes(Span),
  No,
}

#[derive(Clone, Debug)]
pub enum Async {
  Yes(Span),
  No,
}

#[derive(Clone, Debug)]
pub enum Wasm {
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

impl Symbolize for Pattern {
  fn symbolize(&self) -> &Symbol {
    self.kind.symbolize()
  }
}

#[derive(Clone, Debug)]
pub enum PatternKind {
  Underscore,
  Ident(Box<Expr>),
  Lit(Lit),
  MeLower,
}

impl Symbolize for PatternKind {
  fn symbolize(&self) -> &Symbol {
    match self {
      Self::Ident(ident) => ident.symbolize(),
      Self::Lit(lit) => lit.symbolize(),
      _ => unreachable!(),
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct Program {
  pub items: Vec<Item>,
  pub span: Span,
}

impl Program {
  /// no allocation.
  #[inline]
  pub fn new() -> Self {
    Self {
      items: Vec::with_capacity(0usize),
      span: Span::ZERO,
    }
  }

  #[inline]
  pub fn add_item(&mut self, item: Item) {
    self.items.push(item);
  }
}

#[derive(Clone, Debug)]
pub struct Item {
  pub kind: ItemKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
  Load(Load),
  Pack(Pack),
  Var(Var),
  TyAlias(TyAlias),
  Ext(Ext),
  Abstract(Abstract),
  Struct(Struct),
  Apply(Apply),
  Fun(Fun),
}

// ...wip.
#[derive(Clone, Debug)]
pub struct Load {
  pub paths: Vec<Pattern>,
  pub span: Span,
}

// ...wip.
#[derive(Clone, Debug)]
pub struct Pack {}

#[derive(Clone, Debug)]
pub struct Var {
  pub pubness: Pub,
  pub mutability: Mutability,
  pub kind: VarKind,
  pub pattern: Pattern,
  pub maybe_ty: Option<Ty>,
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

#[derive(Clone, Debug)]
pub struct TyAlias {
  pub pubness: Pub,
  pub pattern: Pattern,
  pub maybe_ty: Option<Ty>,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Ext {
  pub pubness: Pub,
  pub prototype: Prototype,
  pub maybe_body: Option<Block>,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Abstract {
  pub pattern: Pattern,
  pub body: Block,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Struct {}

#[derive(Clone, Debug)]
pub struct Apply {}

#[derive(Clone, Debug)]
pub struct Fun {
  pub prototype: Prototype,
  pub body: Block,
  pub span: Span,
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

impl AsSpan for Inputs {
  fn as_span(&self) -> Span {
    let mut iter = self.iter();
    let lhs = iter.next();
    let rhs = iter.last();

    match (lhs, rhs) {
      (Some(lhs), Some(rhs)) => lhs.span.to(rhs.span),
      (Some(lhs), None) => lhs.span,
      (None, Some(rhs)) => rhs.span,
      (None, None) => Span::ZERO,
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
  pub ty: Ty,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum OutputTy {
  Default(Span),
  Ty(Ty),
}

impl AsSpan for OutputTy {
  fn as_span(&self) -> Span {
    match self {
      OutputTy::Default(span) => *span,
      OutputTy::Ty(ty) => ty.span,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Block {
  pub stmts: Vec<Stmt>,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Stmt {
  pub kind: StmtKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
  Var(Var),
  Item(Box<Item>),
  Expr(Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Expr {
  pub kind: ExprKind,
  pub span: Span,
}

impl Symbolize for Expr {
  fn symbolize(&self) -> &Symbol {
    self.kind.symbolize()
  }
}

#[derive(Clone, Debug)]
pub enum ExprKind {
  // lits.
  Lit(Lit),

  // prefix, infix.
  UnOp(UnOp, Box<Expr>),
  BinOp(BinOp, Box<Expr>, Box<Expr>),

  // assignments.
  Assign(Box<Expr>, Box<Expr>),
  AssignOp(BinOp, Box<Expr>, Box<Expr>),

  // collections.
  Array(Vec<Expr>),
  Tuple(Vec<Expr>),

  // blocks.
  Block(Block),

  // accesses.
  ArrayAccess(Box<Expr>, Box<Expr>),
  TupleAccess(Box<Expr>, Box<Expr>),

  // funs.
  Fn(Prototype, Block),
  Call(Box<Expr>, Args),
  Return(Option<Box<Expr>>),

  // branches.
  IfElse(Box<Expr>, Block, Option<Box<Expr>>),
  When(Box<Expr>, Box<Expr>, Box<Expr>),
  Match(Box<Expr>, Vec<Arm>),

  // loops.
  Loop(Block),
  While(Box<Expr>, Block),
  For(For),

  // controls.
  Break(Option<Box<Expr>>),
  Continue,

  // variables.
  Var(Var),

  // definitions.
  StructExpr(StructExpr),
  Chaining(Box<Expr>, Box<Expr>),
}

impl Symbolize for ExprKind {
  fn symbolize(&self) -> &Symbol {
    match self {
      Self::Lit(lit) => lit.symbolize(),
      _ => unreachable!(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Lit {
  pub kind: LitKind,
  pub span: Span,
}

impl Symbolize for Lit {
  fn symbolize(&self) -> &Symbol {
    self.kind.symbolize()
  }
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

impl Symbolize for LitKind {
  fn symbolize(&self) -> &Symbol {
    match self {
      Self::Int(symbol) => symbol,
      Self::Float(symbol) => symbol,
      Self::Char(symbol) => symbol,
      Self::Str(symbol) => symbol,
      Self::Ident(symbol) => symbol,
      _ => unreachable!(),
    }
  }
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
  // Add,
  Neg,
  Not,
}

impl From<Op> for UnOpKind {
  fn from(op: Op) -> Self {
    match op {
      // Op::Plus => Self::Add,
      Op::Minus => Self::Neg,
      Op::Exclamation => Self::Not,
      _ => unreachable!(),
    }
  }
}

impl From<TokenKind> for UnOpKind {
  fn from(kind: TokenKind) -> Self {
    match kind {
      // TokenKind::Op(Op::Plus) => Self::Add,
      TokenKind::Op(Op::Minus) => Self::Neg,
      TokenKind::Op(Op::Exclamation) => Self::Not,
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
  Add,    // +
  Sub,    // -
  Mul,    // *
  Div,    // /
  Rem,    // %
  And,    // &&
  Or,     // ||
  BitXor, // ^
  BitAnd, // &
  BitOr,  // |
  Lt,     // <
  Gt,     // >
  Le,     // <=
  Ge,     // >=
  Eq,     // ==
  Ne,     // !=
  Shl,    // <<
  Shr,    // >>
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
pub struct Args(pub Vec<Arg>);

impl std::ops::Deref for Args {
  type Target = Vec<Arg>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
pub struct Arg {
  pub pattern: Pattern,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StructExpr {
  pub props: Props,
  pub span: Span,
}

pub type Props = Vec<Prop>;

#[derive(Clone, Debug)]
pub struct Prop {
  pub pattern: Pattern,
  pub value: Expr,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct For {
  pub pattern: Box<Expr>,
  pub body: Box<Expr>,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Arm {
  pub pattern: Box<Expr>,
  pub maybe_body: Option<Box<Expr>>,
  pub span: Span,
}
