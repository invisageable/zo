//! ...

use zhoo_ast::ast;

use smol_str::SmolStr;

#[derive(Clone, Debug)]
pub struct Value {
  pub kind: ValueKind,
}

impl Value {
  #[inline]
  pub const fn new(kind: ValueKind) -> Self {
    Self { kind }
  }

  #[inline]
  pub const fn unit() -> Self {
    Self::new(ValueKind::Unit)
  }

  #[inline]
  pub const fn int(int: i64) -> Self {
    Self::new(ValueKind::Int(int))
  }

  #[inline]
  pub const fn float(float: f64) -> Self {
    Self::new(ValueKind::Float(float))
  }

  #[inline]
  pub const fn ident(ident: SmolStr) -> Self {
    Self::new(ValueKind::Ident(ident))
  }

  #[inline]
  pub const fn bool(boolean: bool) -> Self {
    Self::new(ValueKind::Bool(boolean))
  }

  #[inline]
  pub const fn char(ch: char) -> Self {
    Self::new(ValueKind::Char(ch))
  }

  #[inline]
  pub const fn str(string: SmolStr) -> Self {
    Self::new(ValueKind::Str(string))
  }
}

impl std::ops::Add for &Value {
  type Output = i64;

  fn add(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => lhs + rhs,
      _ => unreachable!(),
    }
  }
}

impl std::ops::Sub for &Value {
  type Output = i64;

  fn sub(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => lhs - rhs,
      _ => unreachable!(),
    }
  }
}

impl std::ops::Mul for &Value {
  type Output = i64;

  fn mul(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => lhs * rhs,
      _ => unreachable!(),
    }
  }
}

impl std::ops::Div for &Value {
  type Output = i64;

  fn div(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => lhs / rhs,
      _ => unreachable!(),
    }
  }
}

impl std::ops::Rem for &Value {
  type Output = i64;

  fn rem(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => lhs % rhs,
      _ => unreachable!(),
    }
  }
}

#[derive(Clone, Debug)]
pub enum ValueKind {
  Unit,
  Int(i64),
  Float(f64),
  Ident(SmolStr),
  Bool(bool),
  Char(char),
  Str(SmolStr),
  BinOp(Box<Value>, ast::BinOp, Box<Value>),
}
