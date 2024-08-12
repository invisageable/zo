use zo_interner::interner::symbol::{Symbol, Symbolize};
use zo_tokenizer::token::int::Base;
use zo_tokenizer::token::punctuation::Punctuation;
use zo_tokenizer::token::{Token, TokenKind};
use zo_ty::ty::Ty;

use swisskit::span::{AsSpan, Span};

use smol_str::{SmolStr, ToSmolStr};
use thin_vec::ThinVec;

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

/// The representation of a asyncness.
#[derive(Clone, Debug)]
pub enum Async {
  /// Allows asyncness.
  Yes(Span),
  /// Disallows asyncness.
  No,
}

/// The representation of a wasmness.
#[derive(Clone, Debug)]
pub enum Wasm {
  /// Allows wasmness.
  Yes(Span),
  /// Disallows wasmness.
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

/// The representation of a path.
#[derive(Clone, Debug)]
pub struct Path {
  /// A list of segments — see also [`PathSegment`] for more information.
  pub segments: ThinVec<PathSegment>,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of a path segment.
#[derive(Clone, Debug)]
pub struct PathSegment {
  /// An identifier i.e a path segment.
  pub ident: Ident,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
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
  /// An underscore — `_`.
  Underscore,
  /// A literal pattern.
  Lit(Lit),
  /// An identifier pattern — `foo`, `Bar`.
  Ident(Box<Expr>),
  /// A path pattern.
  Path(Path),
  /// An array destructuring.
  Array(ThinVec<Pattern>),
  /// A tuple destructuring.
  Tuple(ThinVec<Pattern>),
  /// A me.
  Slf(Slf),
}

#[derive(Clone, Debug)]
pub enum Slf {
  /// A lower case self — `self`.
  Lower,
  /// An upper case self — `Self`.
  Upper,
}

impl Symbolize for PatternKind {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    match self {
      Self::Ident(ident) => ident.as_symbol(),
      Self::Lit(lit) => lit.as_symbol(),
      // tmp.
      Self::Array(patterns) if let [pat] = patterns.as_slice() => {
        pat.as_symbol()
      }
      // tmp.
      Self::Tuple(patterns) if let [pat] = patterns.as_slice() => {
        pat.as_symbol()
      }
      _ => unreachable!(),
    }
  }
}

/// The representation of an abstract syntax tree.
#[derive(Clone, Debug, Default)]
pub struct Ast {
  /// The nodes of the AST.
  pub stmts: ThinVec<Stmt>,
}

impl Ast {
  /// Creates a new abstract syntax tree.
  #[inline]
  pub fn new() -> Self {
    Self {
      stmts: ThinVec::with_capacity(0usize),
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
  type Target = ThinVec<Stmt>;

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
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of different kinds of statements.
#[derive(Clone, Debug)]
pub enum ItemKind {
  /// An open item — `open foo;`.
  Open(Open),
  /// An load item — `load foo;`.
  Load(Load),
  /// An alias item — `type Alias = 0;`.
  Alias(Alias),
  /// A constant item i.e global variable — `val x: int = 0;`.
  Var(Var),
  /// An extern function declaration — `ext foo(x: int);`, `ext bar() {..}`.
  Ext(Ext),
  /// An abstract interface — `abstract Foo {..}`.
  Abstract(Abstract),
  /// An enumeration — `enum Foo {..}`, `pub enum Foo {..}`.
  Enum(Enum),
  /// A structure — `struct Foo { x: int }`, `pub struct Foo { x: int }`.
  Struct(Struct),
  /// An apply — `apply Foo {..}`, `apply Foo for Bar {..}`.
  Apply(Apply),
  /// A function — `fun foo(x: int): int {..}`.
  Fun(Fun),
}

#[derive(Clone, Debug)]
pub struct Open {
  /// The path — see also [`Path`] for more information.
  pub path: Path,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Load {
  /// The path — see also [`Path`] for more information.
  pub path: Path,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of an alias.
#[derive(Clone, Debug)]
pub struct Alias {
  /// A publicness — see also [`Pub`] for more information.
  pub pubness: Pub,
  /// A pattern — see also [`Pattern`] for more information.
  pub pattern: Pattern,
  /// A specified type — see also [`Ty`] for more information.
  pub ty: Ty,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of an extern.
#[derive(Clone, Debug)]
pub struct Ext {
  /// A publicness — see also [`Pub`] for more information.
  pub pubness: Pub,
  /// A prototype — see also [`Prototype`] for more information.
  pub prototype: Prototype,
  /// A block — see also [`Block`] for more information.
  pub maybe_block: Option<Block>,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of an abstract.
#[derive(Clone, Debug)]
pub struct Abstract {
  /// A pattern — see also [`Pattern`] for more information.
  pub pattern: Pattern,
  /// A block — see also [`Block`] for more information.
  pub block: Block,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of an enumeration.
#[derive(Clone, Debug)]
pub struct Enum {
  /// A pattern — see also [`Pattern`] for more information.
  pub pattern: Pattern,
  /// A list of variants — see also [`Variant`] for more information.
  pub variants: ThinVec<Variant>,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of an enumeration variant.
#[derive(Clone, Debug)]
pub struct Variant {
  /// The kind variant — `Foo::Bar`, `Bar::Foo(..)`, `Oof::Rab {..}`.
  pub kind: VariantKind,
  /// A pattern — see also [`Pattern`] for more information.
  pub ident: Ident,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of an kind variant.
#[derive(Clone, Debug)]
pub enum VariantKind {
  /// An identifier variant — enum Foo { Bar }.
  Ident,
  /// A tuple variant — `enum Foo { Bar(..) }`.
  Tuple(ThinVec<Expr>),
  /// A struct variant. — `enum Foo { Bar {..} }`.
  Struct(ThinVec<Expr>),
}

/// The representation of a structure.
#[derive(Clone, Debug)]
pub struct Struct {
  /// A pattern — see also [`Pattern`] for more information.
  pub pattern: Pattern,
  /// A list of properties— see also [`Prop`] for more information.
  pub props: Props,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of a structure.
#[derive(Clone, Debug)]
pub struct Prop {
  /// A publicness — see also [`Pub`] for more information.
  pub pubness: Pub,
  /// A name — see also [`Ident`] for more information.
  pub ident: Ident,
  /// A type — see also [`Ty`] for more information.
  pub ty: Ty,
  /// A possible value. A value can be specified by default — see also [`Expr`]
  /// for more information.
  pub maybe_value: Option<Expr>,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of a list of props.
#[derive(Clone, Debug)]
pub struct Props(pub ThinVec<Prop>);

impl std::ops::Deref for Props {
  type Target = ThinVec<Prop>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// The representation of an apply.
#[derive(Clone, Debug)]
pub struct Apply {
  /// A list of statement.
  pub block: ThinVec<Stmt>,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of a function.
#[derive(Clone, Debug)]
pub struct Fun {
  /// A prototype function — see also [`Prototype`] for more information.
  pub prototype: Prototype,
  /// A block — see also [`Block`] for more information.
  pub block: Block,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
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
/// immutable variable — `imu foo: int = 123;`, `imu bar := 123;`.
/// mutable variable — `mut foo: int = 123;`, `mut bar := 123;`.
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
  Array(ThinVec<Expr>),
  /// array access (index) — `foo[0]`.
  ArrayAccess(Box<Expr>, Box<Expr>),
  /// tuple — `(1, 2, 3, 4)`.
  Tuple(ThinVec<Expr>),
  /// tuple access — `foo.0`.
  TupleAccess(Box<Expr>, Box<Expr>),
  /// block — `{..}`.
  Block(Block),
  /// if else — `if foo == 2 {..}`.
  IfElse(Box<Expr>, Block, Option<Box<Expr>>),
  /// ternary — `when true ? foo : bar`.
  When(Box<Expr>, Box<Expr>, Box<Expr>),
  /// A pattern-matching — ``.
  Match(Box<Expr>, ThinVec<Arm>),
  /// loop — `loop {..}`.
  Loop(Block),
  /// while loop — `while foo < 10 {..}`.
  While(Box<Expr>, Block),
  /// A for loop — `for foo := bar {..}`.
  For(For),
  /// exit return — `return`, `return foo`.
  Return(Option<Box<Expr>>),
  /// exit break — `break`, `break foo`.
  Break(Option<Box<Expr>>),
  /// exit continue — `continue`.
  Continue,
  /// variable — `imu foo : int = 0`, `mut foo := 0`.
  Var(Var),
  /// closure — `fn(x) -> x`, `fn() {..}`.
  Closure(Prototype, Block),
  /// A function call — `foo()`, `bar(1, 2)`.
  Call(Box<Expr>, ThinVec<Expr>),
  /// A cast — `foo as s64`.
  Cast(Box<Expr>, Ty),
  /// A range — `1..2`, `x..y`, `foo()..bar()`.
  Range(Option<Box<Expr>>, Option<Box<Expr>>),
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

/// The representation of an identifier.
#[derive(Clone, Copy, Debug)]
pub struct Ident {
  /// A symbol — see also [`Symbol`] for more information.
  pub sym: Symbol,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

impl Symbolize for Ident {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    &self.sym
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
  pub stmts: ThinVec<Stmt>,
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
      stmts: ThinVec::with_capacity(0usize),
      span: Span::ZERO,
    }
  }
}

impl std::ops::Deref for Block {
  type Target = ThinVec<Stmt>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.stmts
  }
}

/// The representation of a prototype — `foo(x: int): int`.
#[derive(Clone, Debug)]
pub struct Prototype {
  /// The name pattern — see also [`Pattern`] for more information.
  pub pattern: Pattern,
  /// The inputs — see also [`Inputs`] for more information.
  pub inputs: Inputs,
  /// The output type — see also [`OutputTy`] for more information.
  pub output_ty: OutputTy,
  /// The span — see also [`Span`] for more information.
  pub span: Span,
}

impl Symbolize for Prototype {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    self.pattern.as_symbol()
  }
}

impl std::ops::Deref for Prototype {
  type Target = Inputs;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inputs
  }
}

/// The representation of an input.
#[derive(Clone, Debug)]
pub struct Input {
  /// The name pattern — see also [`Pattern`]. for more information.
  pub pattern: Pattern,
  /// The type — see also [`Ty`]. for more information.
  pub ty: Ty,
  /// The span — see also [`Span`]. for more information.
  pub span: Span,
}

impl Symbolize for Input {
  #[inline]
  fn as_symbol(&self) -> &Symbol {
    self.pattern.as_symbol()
  }
}

/// The representation of inputs.
#[derive(Clone, Debug)]
pub struct Inputs(ThinVec<Input>);

impl Inputs {
  /// Creates a new input list.
  #[inline]
  pub fn new(inputs: ThinVec<Input>) -> Self {
    Self(inputs)
  }
}

impl AsSpan for Inputs {
  fn as_span(&self) -> Span {
    let maybe_i1 = self.first();
    let maybe_i2 = self.last();

    match (maybe_i1, maybe_i2) {
      (Some(i1), Some(i2)) => Span::merge(i1.span, i2.span),
      (Some(i1), None) => i1.span,
      _ => Span::ZERO,
    }
  }
}

impl std::ops::Deref for Inputs {
  type Target = ThinVec<Input>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.0
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

/// The representation of an pattern-matching arm.
#[derive(Clone, Debug)]
pub struct Arm {
  /// A pattern — see also [`Pattern`] for more information.
  pub pattern: Box<Expr>,
  /// A block — see also [`Block`] for more information.
  pub block: Option<Box<Expr>>,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}

/// The representation of a for loop.
#[derive(Clone, Debug)]
pub struct For {
  /// A pattern — see also [`Pattern`] for more information.
  pub pattern: Pattern,
  /// An iterator — see also [`Expr`] for more information.
  pub iterator: Box<Expr>,
  /// A block — see also [`Block`] for more information.
  pub block: Block,
  /// A span — see also [`Span`] for more information.
  pub span: Span,
}
