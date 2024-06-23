//! ...

use super::builtin::BuiltinFn;

use zo_ast::ast::{Block, Mutability, Pattern, Prototype, Pub, VarKind};

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::span::{AsSpan, Span};

use hashbrown::HashMap;
use smol_str::{SmolStr, ToSmolStr};

#[derive(Clone, Debug)]
pub struct Value {
  pub kind: ValueKind,
  pub span: Span,
}

impl Value {
  pub const UNIT: Self = Self {
    kind: ValueKind::Unit,
    span: Span::ZERO,
  };

  #[inline]
  pub const fn new(kind: ValueKind, span: Span) -> Self {
    Self { kind, span }
  }

  #[inline]
  pub fn unit(span: Span) -> Self {
    Self::new(ValueKind::Unit, span)
  }

  #[inline]
  pub fn int(int: i64, span: Span) -> Self {
    Self::new(ValueKind::Int(int), span)
  }

  #[inline]
  pub fn float(float: f64, span: Span) -> Self {
    Self::new(ValueKind::Float(float), span)
  }

  #[inline]
  pub fn ident(ident: Symbol, span: Span) -> Self {
    Self::new(ValueKind::Ident(ident), span)
  }

  #[inline]
  pub fn bool(boolean: bool, span: Span) -> Self {
    Self::new(ValueKind::Bool(boolean), span)
  }

  #[inline]
  pub fn char(ch: char, span: Span) -> Self {
    Self::new(ValueKind::Char(ch), span)
  }

  #[inline]
  pub fn str(string: SmolStr, span: Span) -> Self {
    Self::new(ValueKind::Str(string), span)
  }

  #[inline]
  pub fn closure(prototype: Prototype, block: Block, span: Span) -> Self {
    Self::new(ValueKind::Fn(prototype, block), span)
  }

  #[inline]
  pub fn ret(value: Value, span: Span) -> Self {
    Self::new(ValueKind::Return(Box::new(value)), span)
  }

  #[inline]
  pub fn array(array: Array, span: Span) -> Self {
    Self::new(ValueKind::Array(array), span)
  }

  #[inline]
  pub fn record(record: HashMap<RecordKey, Value>, span: Span) -> Self {
    Self::new(ValueKind::Record(record), span)
  }

  #[inline]
  pub fn var(var: Var, span: Span) -> Self {
    Self::new(ValueKind::Var(var), span)
  }

  #[inline]
  pub fn fun(prototype: Prototype, block: Block, span: Span) -> Self {
    Self::new(ValueKind::Fn(prototype, block), span)
  }

  #[inline]
  pub fn loop_while(condition: Box<Value>, block: Block, span: Span) -> Self {
    Self::new(ValueKind::While(condition, block), span)
  }

  #[inline]
  pub fn as_bool(&self) -> bool {
    self.kind.as_bool()
  }
}

impl Symbolize for Value {
  fn as_symbol(&self) -> &Symbol {
    self.kind.as_symbol()
  }
}

impl std::ops::Add for &Value {
  type Output = Value;

  fn add(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs + rhs, Span::merge(self.span, span))
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        Value::float(lhs + rhs, Span::merge(self.span, span))
      }
      (ValueKind::Str(lhs), ValueKind::Str(rhs)) => {
        let mut string = String::with_capacity(lhs.len() + rhs.len());

        string.push_str(lhs);
        string.push_str(rhs);

        Value::str(string.to_smolstr(), Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::Sub for &Value {
  type Output = Value;

  fn sub(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs - rhs, Span::merge(self.span, span))
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        Value::float(lhs - rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::Mul for &Value {
  type Output = Value;

  fn mul(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs * rhs, Span::merge(self.span, span))
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        Value::float(lhs * rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::Div for &Value {
  type Output = Value;

  fn div(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs / rhs, Span::merge(self.span, span))
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        Value::float(lhs / rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::Rem for &Value {
  type Output = Value;

  fn rem(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs % rhs, Span::merge(self.span, span))
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        Value::float(lhs % rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::BitAnd for &Value {
  type Output = Value;

  fn bitand(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs & rhs, Span::merge(self.span, span))
      }
      (ValueKind::Bool(lhs), ValueKind::Bool(rhs)) => {
        Value::bool(lhs & rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::BitOr for &Value {
  type Output = Value;

  fn bitor(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs | rhs, Span::merge(self.span, span))
      }
      (ValueKind::Bool(lhs), ValueKind::Bool(rhs)) => {
        Value::bool(lhs | rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::BitXor for &Value {
  type Output = Value;

  fn bitxor(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs ^ rhs, Span::merge(self.span, span))
      }
      (ValueKind::Bool(lhs), ValueKind::Bool(rhs)) => {
        Value::bool(lhs ^ rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::Shl for &Value {
  type Output = Value;

  fn shl(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs << rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

impl std::ops::Shr for &Value {
  type Output = Value;

  fn shr(self, rhs: Self) -> Self::Output {
    let span = rhs.span;

    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs >> rhs, Span::merge(self.span, span))
      }
      _ => unreachable!(),
    }
  }
}

#[derive(Clone, Debug)]
pub enum ValueKind {
  /// unit — `()`.
  Unit,
  /// integer — `1`.
  Int(i64),
  /// float — `1.5`.
  Float(f64),
  /// identifier — `foo`, `Bar`, etc.
  Ident(Symbol),
  /// bool — `false` or `true`.
  Bool(bool),
  /// character — `'a'`.
  Char(char),
  /// string — `"foobar"`.
  Str(SmolStr),
  /// closure — `fn (x) -> x`, `fn (x) {..}`.
  Fn(Prototype, Block),
  /// return — `return foobar;`, `return;`.
  Return(Box<Value>),
  /// builtin function.
  Builtin(BuiltinFn),
  /// array — `[1, 2, 3, 4]`.
  Array(Array),
  /// float — `{ x = 1, y = 1}`.
  Record(HashMap<RecordKey, Value>),
  /// variable — `imu foo = 1;`, `mut bar = 1`.
  Var(Var),
  /// function — `fun foo() {}`.
  Fun(Prototype, Block),
  /// while instruction value — `while true {..}`.
  While(Box<Value>, Block),
}

impl ValueKind {
  #[inline]
  pub fn as_bool(&self) -> bool {
    match self {
      Self::Bool(boolean) => *boolean,
      Self::Unit => false,
      _ => false,
    }
  }
}

impl Symbolize for ValueKind {
  fn as_symbol(&self) -> &Symbol {
    match self {
      Self::Fn(prototype, _) => prototype.as_symbol(),
      Self::Var(var) => var.pattern.as_symbol(),
      Self::Fun(prototype, _) => prototype.as_symbol(),
      _ => todo!(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Args(pub Vec<Arg>);

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

impl AsSpan for Args {
  fn as_span(&self) -> Span {
    let lo = self.0.first();
    let hi = self.0.last();

    match (lo, hi) {
      (Some(first), Some(last)) => Span::merge(first.span, last.span),
      (Some(first), None) => first.span,
      _ => Span::ZERO,
    }
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
  pub value: Value,
  pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Array(pub Vec<Value>);

impl Array {
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
  pub fn add_elmt(&mut self, value: Value) {
    self.0.push(value)
  }
}

impl Default for Array {
  fn default() -> Self {
    Self::new()
  }
}

impl std::ops::Deref for Array {
  type Target = Vec<Value>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum RecordKey {
  /// integer key.
  Ident(Symbol),
}

impl From<&Value> for RecordKey {
  fn from(value: &Value) -> RecordKey {
    match &value.kind {
      ValueKind::Ident(symbol) => RecordKey::Ident(*symbol),
      _ => unreachable!(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Var {
  pub pubness: Pub,
  pub mutability: Mutability,
  pub kind: VarKind,
  pub pattern: Pattern,
  pub value: Box<Value>,
  pub span: Span,
}
