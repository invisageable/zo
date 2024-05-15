use smol_str::SmolStr;

#[derive(Debug)]
pub struct Expr {
  pub kind: ExprKind,
}

#[derive(Debug)]
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
}

#[derive(Debug)]
pub struct Lit {
  pub kind: LitKind,
}

#[derive(Debug)]
pub enum LitKind {
  Int(i64),
  Float(f64),
  Ident(SmolStr),
  Bool(bool),
  Char(SmolStr),
  Str(SmolStr),
}

#[derive(Debug)]
pub struct UnOp {
  pub kind: UnOpKind,
}

#[derive(Debug)]
pub enum UnOpKind {
  /// negative — `-`
  Neg,
  /// not — `!`
  Not,
}

#[derive(Debug)]
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
