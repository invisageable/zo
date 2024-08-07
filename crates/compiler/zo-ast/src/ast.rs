use zo_interner::interner::symbol::{Symbol, Symbolize};
use zo_tokenizer::token::int::Base;
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};
use zo_ty::ty::Ty;

use swisskit::span::{AsSpan, Span};

use smol_str::{SmolStr, ToSmolStr};

/// The representation of an unique id of a node in an AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// The representation of a public access.
#[derive(Clone, Debug)]
pub enum Pub {
  /// Allows public access.
  Yes(Span),
  /// Disallows access.
  No,
}

/// The representation of a mutability.
#[derive(Clone, Debug)]
pub enum Mutability {
  /// Allows immutable.
  Yes,
  /// Disallows mutable.
  No,
}

/// The representation of a pattern.
#[derive(Clone, Debug)]
pub struct Pattern {
  /// A pattern kind — see also [`PatternKind`] for more information..
  pub kind: PatternKind,
  /// A span of the pattern — see also [`Span`] for more information.
  pub span: Span,
}

impl Symbolize for Pattern {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    self.kind.as_symbol()
  }
}

/// The representation of different kinds of patterns.
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
  #[inline]
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
  /// The nodes of the AST.
  pub stmts: Vec<Stmt>,
}

impl Ast {
  /// Creates a new abstract syntax tree.
  #[inline]
  pub fn new() -> Self {
    Self {
      stmts: Vec::with_capacity(0usize),
    }
  }

  /// Checks if the AST is empty, means that it contains zero statements.
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.stmts.is_empty()
  }

  /// Adds a new statement.
  #[inline]
  pub fn add_stmt(&mut self, stmt: Stmt) {
    self.stmts.push(stmt);
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

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.stmts
  }
}

/// The representation of an item.
#[derive(Clone, Debug)]
pub struct Item {
  /// An item kind — see also [`ItemKind`] if needed.
  pub kind: ItemKind,
  /// An item span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of different kinds of statements.
#[derive(Clone, Debug)]
pub enum ItemKind {
  /// A constant, global variable.
  Var(Var),
}

/// The representation of a statement.
#[derive(Clone, Debug)]
pub struct Stmt {
  /// See [`StmtKind`].
  pub kind: StmtKind,
  /// See [`Span`].
  pub span: Span,
}

/// The representation of different kinds of statements.
#[derive(Clone, Debug)]
pub enum StmtKind {
  /// A variable statement.
  Var(Var),
  /// An item statement.
  Item(Item),
  /// An expression statement.
  Expr(Box<Expr>),
}

/// The representation of a variable.
///
/// immutable variable — `imu foo: int = 123;`.
/// mutable variable — `mut foo: int = 123;`, `mut foo := 123;`.
#[derive(Clone, Debug)]
pub struct Var {
  /// An indicator to delimit the scope.
  pub pubness: Pub,
  /// An indicator to define the mutability.
  pub mutability: Mutability,
  /// A variable kind.
  pub kind: VarKind,
  /// A pattern of a variable — see also [`Pattern`] for more information.
  pub pattern: Pattern,
  /// The type of the variable.
  pub ty: Ty,
  /// The value of the variable.
  pub value: Box<Expr>,
  /// The variable span.
  pub span: Span,
}

impl Symbolize for Var {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    self.pattern.as_symbol()
  }
}

/// The representation of different kinds of variable.
#[derive(Clone, Debug)]
pub enum VarKind {
  /// A global constant variable.
  Val,
  /// An immutable local variable.
  Imu,
  /// A mutable local variable.
  Mut,
}

impl From<VarKind> for SmolStr {
  #[inline]
  fn from(kind: VarKind) -> Self {
    kind.to_smolstr()
  }
}

/// The representation of an expression.
#[derive(Clone, Debug)]
pub struct Expr {
  /// See [`ExprKind`].
  pub kind: ExprKind,
  /// See [`Span`].
  pub span: Span,
}

impl Symbolize for Expr {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    self.kind.as_symbol()
  }
}

/// The representation of different kinds of expressions.
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
  /// array — `[1, 2, 3, 4]`.
  Array(Vec<Expr>),
  /// array access (index) — `foo[0]`.
  ArrayAccess(Box<Expr>, Box<Expr>),
  /// tuple — `(1, 2, 3, 4)`.
  Tuple(Vec<Expr>),
  /// tuple access — `foo.0`.
  TupleAccess(Box<Expr>, Box<Expr>),
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
  /// closure — `fn(x) -> x`, `fn() { .. }`
  Closure(Prototype, Block),
  /// call — `foo()`, `bar(1, 2)`
  Call(Box<Expr>, Vec<Expr>),
}

impl Symbolize for ExprKind {
  #[inline]
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
  /// See [`LitKind`].
  pub kind: LitKind,
  /// See [`Span`].
  pub span: Span,
}

impl Symbolize for Lit {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    self.kind.as_symbol()
  }
}

/// The representation of different kinds of literals.
#[derive(Clone, Debug)]
pub enum LitKind {
  /// integer — `1`.
  Int(Symbol, Base),
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
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    match self {
      Self::Int(symbol, _)
      | Self::Float(symbol)
      | Self::Ident(symbol)
      | Self::Char(symbol)
      | Self::Str(symbol) => symbol,
      _ => unreachable!(),
    }
  }
}

/// The representation of a unary operator.
#[derive(Clone, Copy, Debug)]
pub struct UnOp {
  /// See [`UnOpKind`].
  pub kind: UnOpKind,
  /// See [`Span`].
  pub span: Span,
}

impl From<UnOp> for SmolStr {
  #[inline]
  fn from(unop: UnOp) -> Self {
    unop.to_smolstr()
  }
}

/// The representation of different kinds of unary operators.
#[derive(Clone, Copy, Debug)]
pub enum UnOpKind {
  /// negation operator — `-`.
  Neg,
  /// logical inversion operator — `!`.
  Not,
}

/// The representation of a binary operator.
#[derive(Clone, Copy, Debug)]
pub struct BinOp {
  /// See [`BinOpKind`].
  pub kind: BinOpKind,
  /// See [`Span`].
  pub span: Span,
}

impl From<BinOp> for SmolStr {
  #[inline]
  fn from(binop: BinOp) -> Self {
    binop.to_smolstr()
  }
}

impl From<&Token> for BinOp {
  #[inline]
  fn from(token: &Token) -> Self {
    match token.kind {
      TokenKind::Punctuation(punctuation) => Self {
        kind: BinOpKind::from(punctuation),
        span: token.span,
      },
      _ => unreachable!(),
    }
  }
}

/// The representation of different kinds of binary operators.
#[derive(Clone, Copy, Debug)]
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

impl From<Punctuation> for BinOpKind {
  #[inline]
  fn from(punctuation: Punctuation) -> Self {
    match punctuation {
      Punctuation::Plus => Self::Add,
      Punctuation::Minus => Self::Sub,
      Punctuation::Asterisk => Self::Mul,
      Punctuation::Slash => Self::Div,
      Punctuation::Percent => Self::Rem,
      Punctuation::AmpersandAmpersand => Self::And,
      Punctuation::PipePipe => Self::Or,
      Punctuation::Circumflex => Self::BitXor,
      Punctuation::Ampersand => Self::BitAnd,
      Punctuation::Pipe => Self::BitOr,
      Punctuation::LessThan => Self::Lt,
      Punctuation::GreaterThan => Self::Gt,
      Punctuation::LessThanEqual => Self::Le,
      Punctuation::GreaterThanEqual => Self::Ge,
      Punctuation::EqualEqual => Self::Eq,
      Punctuation::ExclamationEqual => Self::Ne,
      Punctuation::LessThanLessThan => Self::Shl,
      Punctuation::GreaterThanGreaterThan => Self::Shr,
      _ => unreachable!(),
    }
  }
}

/// The representation of a block — `{ .. }`.
#[derive(Clone, Debug)]
pub struct Block {
  /// The statement list inside the block.
  pub stmts: Vec<Stmt>,
  /// The span of a block — see also [`Span`] if your needed.
  pub span: Span,
}

impl Block {
  /// Checks if the block do not constains statement instructions.
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.stmts.is_empty()
  }
}

impl Default for Block {
  #[inline]
  fn default() -> Self {
    Self {
      stmts: Vec::with_capacity(0usize),
      span: Span::ZERO,
    }
  }
}

impl std::ops::Deref for Block {
  type Target = Vec<Stmt>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.stmts
  }
}

/// The representation of a prototype — `foo(x: int): int`.
#[derive(Clone, Debug)]
pub struct Prototype {
  pub pattern: Pattern,
  pub inputs: Vec<Input>,
  pub output_ty: OutputTy,
  pub span: Span,
}

impl Symbolize for Prototype {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    self.pattern.as_symbol()
  }
}

#[derive(Clone, Debug)]
pub struct Input {
  pub pattern: Pattern,
  pub ty: Ty,
  pub span: Span,
}

impl Symbolize for Input {
  #[inline]
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
  #[inline]
  fn as_span(&self) -> Span {
    match self {
      Self::Default(span) => *span,
      Self::Ty(ty) => ty.span,
    }
  }
}
