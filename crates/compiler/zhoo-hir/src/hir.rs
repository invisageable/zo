//! ...

use zo_core::{interner::symbol::Symbol, span::Span};

#[derive(Clone, Copy, Debug)]
pub enum Pub {
  Yes(Span),
  No,
}

#[derive(Clone, Copy, Debug)]
pub enum Async {
  Yes(Span),
  No,
}

#[derive(Clone, Copy, Debug)]
pub enum Wasm {
  Yes(Span),
  No,
}

#[derive(Clone, Copy, Debug)]
pub enum Mutability {
  Yes(Span),
  No,
}

#[derive(Clone, Copy, Debug)]
pub struct Pattern<'hir> {
  pub kind: PatternKind<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum PatternKind<'hir> {
  Underscore,
  Ident(&'hir Expr<'hir>),
  Lit(&'hir Lit),
  MeLower,
}

#[derive(Clone, Copy, Debug)]
pub struct Hir<'hir> {
  pub items: &'hir [Item<'hir>],
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Item<'hir> {
  pub kind: ItemKind<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum ItemKind<'hir> {
  Load(&'hir Load<'hir>),
  Pack(&'hir Pack),
  Var(&'hir Var<'hir>),
  TyAlias(&'hir TyAlias<'hir>),
  Ext(&'hir Ext<'hir>),
  Abstract(&'hir Abstract<'hir>),
  Enum(&'hir Enum<'hir>),
  Struct(&'hir Struct),
  Apply(&'hir Apply),
  Fun(&'hir Fun<'hir>),
}

#[derive(Clone, Copy, Debug)]
pub struct Load<'hir> {
  pub paths: &'hir [Pattern<'hir>],
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Pack {}

#[derive(Clone, Copy, Debug)]
pub struct Var<'hir> {
  pub pubness: Pub,
  pub mutability: Mutability,
  pub kind: VarKind,
  pub pattern: Pattern<'hir>,
  // pub maybe_ty: Option<Ty>,
  pub value: &'hir Expr<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum VarKind {
  Imu,
  Mut,
  Val,
}

#[derive(Clone, Copy, Debug)]
pub struct TyAlias<'hir> {
  pub pubness: Pub,
  pub pattern: Pattern<'hir>,
  // pub maybe_ty: Option<Ty>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Ext<'hir> {
  pub pubness: Pub,
  pub prototype: Prototype<'hir>,
  pub maybe_body: Option<&'hir Block<'hir>>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Abstract<'hir> {
  pub pattern: Pattern<'hir>,
  pub body: &'hir Block<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Enum<'hir> {
  pub pattern: Pattern<'hir>,
  pub body: &'hir [Variant],
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Variant {}

#[derive(Clone, Copy, Debug)]
pub struct Struct {}

#[derive(Clone, Copy, Debug)]
pub struct Apply {}

#[derive(Clone, Copy, Debug)]
pub struct Fun<'hir> {
  pub prototype: Prototype<'hir>,
  pub blokc: Block<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Prototype<'hir> {
  pub pattern: Pattern<'hir>,
  pub inputs: Inputs<'hir>,
  pub output_ty: OutputTy,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum OutputTy {
  Default(Span),
  // Ty(Ty),
}

#[derive(Clone, Copy, Debug)]
pub struct Inputs<'hir>(pub &'hir [Input<'hir>]);

#[derive(Clone, Copy, Debug)]
pub struct Input<'hir> {
  pub pattern: Pattern<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Block<'hir> {
  pub stmts: &'hir [Stmt<'hir>],
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Stmt<'hir> {
  pub kind: StmtKind<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum StmtKind<'hir> {
  Item(Item<'hir>),
  Expr(Expr<'hir>),
}

#[derive(Clone, Copy, Debug)]
pub struct Expr<'hir> {
  pub kind: ExprKind<'hir>,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum ExprKind<'hir> {
  // literals.
  Lit(&'hir Lit),

  // prefix, infix.
  UnOp(UnOp, &'hir Expr<'hir>),
  BinOp(BinOp, &'hir Expr<'hir>, &'hir Expr<'hir>),

  // collections.
  Array(&'hir [Expr<'hir>]),
  Tuple(&'hir [Expr<'hir>]),

  // accesses.
  ArrayAccess(&'hir Expr<'hir>, &'hir Expr<'hir>),
  TupleAccess(&'hir Expr<'hir>, &'hir Expr<'hir>),

  // blocks.
  Block(&'hir Block<'hir>),

  // funs.
  Fn(&'hir Prototype<'hir>, &'hir Block<'hir>),
  Call(&'hir Expr<'hir>, &'hir Args<'hir>),

  // branches.
  IfElse(&'hir Expr<'hir>, &'hir Expr<'hir>, Option<&'hir Expr<'hir>>),
  When(&'hir Expr<'hir>, &'hir Expr<'hir>, &'hir Expr<'hir>),

  // loops.
  Loop(&'hir Block<'hir>),
  While(&'hir Expr<'hir>, &'hir Block<'hir>),

  // controls.
  Return(Option<&'hir Expr<'hir>>),
  Break(Option<&'hir Expr<'hir>>),
  Continue,

  // definitions.
  Chaining(&'hir Expr<'hir>, &'hir Expr<'hir>),
}

#[derive(Clone, Copy, Debug)]
pub struct Lit {
  pub kind: LitKind,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum LitKind {
  Int(Symbol),
  Float(Symbol),
  Ident(Symbol),
  Bool(bool),
  Char(Symbol),
  Str(Symbol),
}

#[derive(Clone, Copy, Debug)]
pub struct UnOp {
  pub kind: UnOpKind,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum UnOpKind {
  Neg,
  Not,
}

#[derive(Clone, Copy, Debug)]
pub struct BinOp {
  pub kind: BinOpKind,
  pub span: Span,
}

#[derive(Clone, Copy, Debug)]
pub enum BinOpKind {
  Add,    // +
  Sub,    // -
  Mul,    // *
  Div,    // /
  Rem,    // %
  And,    // &&
  Or,     // ||
  BitAnd, // &
  BitOr,  // |
  BitXor, // ^
  Lt,     // <
  Gt,     // >
  Le,     // <=
  Ge,     // >=
  Eq,     // ==
  Ne,     // !=
  Shl,    // <<
  Shr,    // >>
  Range,  // ..
}

#[derive(Clone, Copy, Debug)]
pub struct Args<'hir>(pub &'hir [Arg<'hir>]);

impl Args<'_> {
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

impl<'hir> std::ops::Deref for Args<'hir> {
  type Target = &'hir [Arg<'hir>];

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Arg<'hir> {
  pub pattern: Pattern<'hir>,
  // pub ty: Ty,
  pub span: Span,
}
