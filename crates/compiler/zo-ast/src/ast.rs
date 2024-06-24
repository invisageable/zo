//! The `zo` Abstract Syntax Tree module.

use zo_tokenizer::token::kw::Kw;
use zo_tokenizer::token::op::Op;
use zo_tokenizer::token::{Token, TokenKind};
use zo_ty::ty::Ty;

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::span::{AsSpan, Span};

/// The representation of an node id in an AST node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

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

impl Symbolize for Pattern {
  fn as_symbol(&self) -> &Symbol {
    self.kind.as_symbol()
  }
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

impl Symbolize for PatternKind {
  fn as_symbol(&self) -> &Symbol {
    match self {
      Self::Ident(ident) => ident.as_symbol(),
      Self::Lit(lit) => lit.as_symbol(),
      _ => unreachable!(),
    }
  }
}

/// The representation of an abstract syntax tree.
#[derive(Clone, Debug, Default)]
pub struct Ast {
  pub stmts: Vec<Stmt>,
}

impl Ast {
  /// no allocation.
  #[inline]
  pub fn new() -> Self {
    Self {
      stmts: Vec::with_capacity(0usize),
    }
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.stmts.is_empty()
  }

  #[inline]
  pub fn add_stmt(&mut self, stmt: Stmt) {
    self.stmts.push(stmt);
  }
}

impl AsSpan for Ast {
  fn as_span(&self) -> Span {
    let lo = self.stmts.first();
    let hi = self.stmts.last();

    match (lo, hi) {
      (Some(first), Some(last)) => Span::merge(first.span, last.span),
      (Some(first), None) => first.span,
      _ => Span::ZERO,
    }
  }
}

impl std::convert::AsMut<Self> for Ast {
  #[inline]
  fn as_mut(&mut self) -> &mut Self {
    self
  }
}

impl std::ops::Deref for Ast {
  type Target = Vec<Stmt>;

  fn deref(&self) -> &Self::Target {
    &self.stmts
  }
}

/// The representation of an item.
#[derive(Clone, Debug)]
pub struct Item {
  pub kind: ItemKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
  /// load module — `load ".."`.
  Load(Load),
  /// `val FOO: int = 123;`, `pub val FOO: int = 123;`.
  Var(Var),
  /// `type Foo = int;`, `pub type Foo = int;`.
  TyAlias(TyAlias),
  /// `ext foobar(x: int);`, `ext foobar(x: int) { .. }`.
  Ext(Ext),
  /// `struct Foo { x: int }`, `pub struct Foo { x: int }`.
  Struct(Struct),
  /// `fun foo(x: int): int { .. }`, `pub fun foo(x: int): int { .. }`.
  Fun(Fun),
}

#[derive(Clone, Debug)]
pub struct Load {
  pub ast: Ast,
  pub span: Span,
}

/// The representation of an type alias — `type Foo = int`.
#[derive(Clone, Debug)]
pub struct TyAlias {
  pub pubness: Pub,
  pub pattern: Pattern,
  pub maybe_ty: Option<Ty>,
  pub span: Span,
}

/// The representation of a extern — `ext foo() { .. }`.
#[derive(Clone, Debug)]
pub struct Ext {
  pub pubness: Pub,
  pub prototype: Prototype,
  pub maybe_body: Option<Block>,
  pub span: Span,
}

/// The representation of a structure — `struct Foo { .. }`.
#[derive(Clone, Debug)]
pub struct Struct {
  pub ident: Ident,
  pub fields: Fields,
  pub span: Span,
}

/// The representation of fields in a structure.
#[derive(Clone, Debug)]
pub struct Fields(pub Vec<Field>);

impl Fields {
  /// no allocation.
  #[inline]
  pub fn new() -> Self {
    Self(Vec::with_capacity(0usize))
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  #[inline]
  pub fn add_field(&mut self, field: Field) {
    self.0.push(field)
  }
}

impl Default for Fields {
  fn default() -> Self {
    Self::new()
  }
}

impl std::ops::Deref for Fields {
  type Target = Vec<Field>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// The representation of field.
///
/// `x: int` or `x = 0`.
#[derive(Clone, Debug)]
pub struct Field {
  pub ident: Ident,
  pub ty: Ty,
  pub span: Span,
}

/// The representation of a extern — `fun foo() { .. }`.
#[derive(Clone, Debug)]
pub struct Fun {
  pub prototype: Prototype,
  pub body: Block,
  pub span: Span,
}

/// The representation of a statement.
#[derive(Clone, Debug)]
pub struct Stmt {
  pub kind: StmtKind,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
  /// variable.
  Var(Var),
  /// item.
  Item(Box<Item>),
  /// expression.
  Expr(Box<Expr>),
}

/// The representation of a variable.
///
/// immutable variable — `imu foo: int = 123;`.
/// mutable variable — `mut foo: int = 123;`, `mut foo := 123;`.
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

impl Symbolize for Var {
  fn as_symbol(&self) -> &Symbol {
    self.pattern.as_symbol()
  }
}

#[derive(Clone, Debug)]
pub enum VarKind {
  /// immutable.
  Imu,
  /// mutable.
  Mut,
  /// constant.
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

/// The representation of an expression.
#[derive(Clone, Debug)]
pub struct Expr {
  pub kind: ExprKind,
  pub span: Span,
}

impl Symbolize for Expr {
  fn as_symbol(&self) -> &Symbol {
    self.kind.as_symbol()
  }
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
  /// block — `{ .. }`.
  Block(Block),
  /// closure — `fn() -> ..`, `fn() { .. }`.
  Fn(Prototype, Block),
  /// call function — `foo()`, `bar(1, 2)`.
  Call(Box<Expr>, Args),
  /// array — `[1, 2, 3, 4]`.
  Array(Vec<Expr>),
  /// array access (index) — `foo[0]`.
  ArrayAccess(Box<Expr>, Box<Expr>),
  /// structure — `{ x = 1, y = 2 }`.
  Struct(StructExpr),
  /// structure access (dot) — `foo.x`.
  StructAccess(Box<Expr>, Box<Expr>),
  /// if else — `if foo == 2 { .. }`.
  IfElse(Box<Expr>, Block, Option<Box<Expr>>),
  /// ternary — `when true ? foo : bar`.
  When(Box<Expr>, Box<Expr>, Box<Expr>),
  /// loop — `loop { .. }`.
  Loop(Block),
  /// while loop — `while foo < 10 { .. }`.
  While(Box<Expr>, Block),
  /// exit return — `return`, `return foo`.
  Return(Option<Box<Expr>>),
  /// exit break — `break`, `break foo`.
  Break(Option<Box<Expr>>),
  /// exit continue — `continue`.
  Continue,
  /// variable — `imu foo : int = 0`, `mut foo := 0`.
  Var(Var),
}

impl Symbolize for ExprKind {
  fn as_symbol(&self) -> &Symbol {
    match self {
      Self::Lit(lit) => lit.as_symbol(),
      _ => todo!(),
    }
  }
}

/// The representation of a literal.
#[derive(Clone, Debug)]
pub struct Lit {
  pub kind: LitKind,
  pub span: Span,
}

impl Symbolize for Lit {
  fn as_symbol(&self) -> &Symbol {
    self.kind.as_symbol()
  }
}

#[derive(Clone, Debug)]
pub enum LitKind {
  /// integer — `1`.
  Int(Symbol),
  /// integer — `1.2`, `1e4`.
  Float(Symbol),
  /// identifier — `foo`, `Bar`, `foobar1234`.
  Ident(Symbol),
  /// boolean — `false`, `true`.
  Bool(bool),
  /// character — `'\0'`.
  Char(Symbol),
  /// string — `"foobar"`.
  Str(Symbol),
}

impl Symbolize for LitKind {
  fn as_symbol(&self) -> &Symbol {
    match self {
      Self::Int(symbol) => symbol,
      Self::Float(symbol) => symbol,
      Self::Ident(symbol) => symbol,
      Self::Char(symbol) => symbol,
      Self::Str(symbol) => symbol,
      _ => unreachable!(),
    }
  }
}

/// The representation of a unary operator.
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
  /// negation operator — `-`.
  Neg,
  /// logical inversion operator — `!`.
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

/// The representation of a binary operator.
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
  /// addition operator — `+`.
  Add,
  /// subtraction operator — `-`.
  Sub,
  /// multiplication operator — `*`.
  Mul,
  /// division operator — `/`.
  Div,
  /// modulus operator — `%`.
  Rem,
  /// logical and operator — `&&`.
  And,
  /// logical or operator — `||`.
  Or,
  /// bitwise and operator — `&`.
  BitAnd,
  /// bitwise or operator — `|`.
  BitOr,
  /// bitwise xor operator — `^`.
  BitXor,
  /// less than operator — `<`.
  Lt,
  /// greater than operator — `>`.
  Gt,
  /// less than or equal operator — `<=`.
  Le,
  /// greater than or equal operator — `>=`.
  Ge,
  /// equality operator — `==`.
  Eq,
  /// not equal operator — `!=`.
  Ne,
  /// shift left operator — `<<`.
  Shl,
  /// shift right operator — `>>`.
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

/// The representation of a block — `{..}`.
#[derive(Clone, Debug)]
pub struct Block {
  pub stmts: Vec<Stmt>,
  pub span: Span,
}

impl Block {
  /// no allocation.
  #[inline]
  pub fn new() -> Self {
    Self {
      stmts: Vec::with_capacity(0usize),
      span: Span::ZERO,
    }
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.stmts.is_empty()
  }

  #[inline]
  pub fn add_stmt(&mut self, stmt: Stmt) {
    self.stmts.push(stmt)
  }
}

impl AsSpan for Block {
  fn as_span(&self) -> Span {
    let lo = self.stmts.first();
    let hi = self.stmts.last();

    match (lo, hi) {
      (Some(first), Some(last)) => Span::merge(first.span, last.span),
      (Some(first), None) => first.span,
      _ => Span::ZERO,
    }
  }
}

impl Default for Block {
  fn default() -> Self {
    Self::new()
  }
}

impl std::ops::Deref for Block {
  type Target = Vec<Stmt>;

  fn deref(&self) -> &Self::Target {
    &self.stmts
  }
}

/// The representation of a prototype — `foo(x: int): int`.
#[derive(Clone, Debug)]
pub struct Prototype {
  pub pattern: Pattern,
  pub inputs: Inputs,
  pub output_ty: OutputTy,
  pub span: Span,
}

impl Symbolize for Prototype {
  fn as_symbol(&self) -> &Symbol {
    self.pattern.as_symbol()
  }
}

#[derive(Clone, Debug)]
pub struct Inputs(pub Vec<Input>);

impl Inputs {
  /// no allocation.
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

impl Default for Inputs {
  fn default() -> Self {
    Self::new()
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

impl Symbolize for Input {
  fn as_symbol(&self) -> &Symbol {
    self.pattern.as_symbol()
  }
}

/// The representation of an output type.
#[derive(Clone, Debug)]
pub enum OutputTy {
  /// returns `()`.
  Default(Span),
  /// returns type such as `int`, float, etc.
  Ty(Ty),
}

impl AsSpan for OutputTy {
  fn as_span(&self) -> Span {
    match self {
      Self::Default(span) => *span,
      Self::Ty(ty) => ty.span,
      _ => unreachable!(),
    }
  }
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
  /// no allocation.
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

impl Default for Args {
  fn default() -> Self {
    Self::new()
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

/// The representation of a structure expression — `{ x = 1, y = 1 }`.
#[derive(Clone, Debug)]
pub struct StructExpr {
  pub pairs: Vec<(Expr, Expr)>,
}

/// The representation of an identifier.
#[derive(Copy, Clone, Debug)]
pub struct Ident {
  pub name: Symbol,
  pub span: Span,
}
