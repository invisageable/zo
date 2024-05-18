use smol_str::SmolStr;

#[derive(Clone, Debug)]
pub struct Pattern {
  pub kind: PatternKind,
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

#[derive(Clone, Debug)]
pub struct Expr {
  pub kind: ExprKind,
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
}

#[derive(Clone, Debug)]
pub struct Lit {
  pub kind: LitKind,
}

#[derive(Clone, Debug)]
pub enum LitKind {
  Int(i64),
  Float(f64),
  Ident(SmolStr),
  Bool(bool),
  Char(SmolStr),
  Str(SmolStr),
}

#[derive(Clone, Debug)]
pub struct UnOp {
  pub kind: UnOpKind,
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
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.exprs.is_empty()
  }
}

#[derive(Clone, Debug)]
pub struct Prototype {
  pub pattern: Pattern,
  pub inputs: Inputs,
  pub output_ty: OutputTy,
}

#[derive(Clone, Debug)]
pub struct Inputs(pub Vec<Input>);

impl std::ops::Deref for Inputs {
  type Target = Vec<Input>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
pub struct Input {
  pub pattern: Pattern,
}

#[derive(Clone, Debug)]
pub enum OutputTy {
  // Default(Span),
  // Ty(Ty),
}
