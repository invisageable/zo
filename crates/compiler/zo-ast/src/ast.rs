use zo_interner::interner::symbol::{Symbol, Symbolize};
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};

use swisskit::span::Span;

/// The representation of an unique id of a node in an AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

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
  /// The expression statement.
  Expr(Box<Expr>),
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
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    match self {
      Self::Int(symbol)
      | Self::Float(symbol)
      | Self::Ident(symbol)
      | Self::Char(symbol)
      | Self::Str(symbol) => symbol,
      _ => unreachable!(),
    }
  }
}

/// The representation of a unary operator.
#[derive(Clone, Debug)]
pub struct UnOp {
  /// See [`UnOpKind`].
  pub kind: UnOpKind,
  /// See [`Span`].
  pub span: Span,
}

/// The representation of different kinds of unary operators.
#[derive(Clone, Debug)]
pub enum UnOpKind {
  /// negation operator — `-`.
  Neg,
  /// logical inversion operator — `!`.
  Not,
}

/// The representation of a binary operator.
#[derive(Clone, Debug)]
pub struct BinOp {
  /// See [`BinOpKind`].
  pub kind: BinOpKind,
  /// See [`Span`].
  pub span: Span,
}

impl From<&Token> for BinOp {
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

impl From<Punctuation> for BinOpKind {
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
