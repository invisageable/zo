//! ...

use zhoo_ast::ast;

use smol_str::SmolStr;

#[derive(Clone, Debug)]
pub struct Value {
  pub kind: ValueKind,
}

impl Value {
  #[inline]
  pub const fn of(kind: ValueKind) -> Self {
    Self { kind }
  }

  #[inline]
  pub const fn unit() -> Self {
    Self::of(ValueKind::Unit)
  }

  #[inline]
  pub const fn int(int: i64) -> Self {
    Self::of(ValueKind::Int(int))
  }

  #[inline]
  pub const fn float(float: f64) -> Self {
    Self::of(ValueKind::Float(float))
  }

  #[inline]
  pub const fn ident(ident: SmolStr) -> Self {
    Self::of(ValueKind::Ident(ident))
  }

  #[inline]
  pub const fn bool(boolean: bool) -> Self {
    Self::of(ValueKind::Bool(boolean))
  }

  #[inline]
  pub const fn char(ch: char) -> Self {
    Self::of(ValueKind::Char(ch))
  }

  #[inline]
  pub const fn str(string: SmolStr) -> Self {
    Self::of(ValueKind::Str(string))
  }
}

impl std::ops::Add for &Value {
  type Output = Value;

  fn add(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => Value::int(lhs + rhs),
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => Value::float(lhs + rhs),
      _ => unreachable!(),
    }
  }
}

impl std::ops::Sub for &Value {
  type Output = Value;

  fn sub(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => Value::int(lhs - rhs),
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => Value::float(lhs - rhs),
      _ => unreachable!(),
    }
  }
}

impl std::ops::Mul for &Value {
  type Output = Value;

  fn mul(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => Value::int(lhs * rhs),
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => Value::float(lhs * rhs),
      _ => unreachable!(),
    }
  }
}

impl std::ops::Div for &Value {
  type Output = Value;

  fn div(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => Value::int(lhs / rhs),
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => Value::float(lhs / rhs),
      _ => unreachable!(),
    }
  }
}

impl std::ops::Rem for &Value {
  type Output = Value;

  fn rem(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => Value::int(lhs % rhs),
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
  UnOp(ast::UnOp, Box<Value>),
  BinOp(ast::BinOp, Box<Value>, Box<Value>),
}
