//! ...

use super::builtin::BuiltinFn;

use zo_ast::ast::{Block, Prototype};

use zo_core::interner::symbol::Symbol;
use zo_core::span::{AsSpan, Span};

use smol_str::SmolStr;

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
  pub fn new(kind: ValueKind, span: Span) -> Self {
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
  pub fn fun(prototype: Prototype, block: Block, span: Span) -> Self {
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
  pub fn symbolize(&self) -> Symbol {
    self.kind.symbolize()
  }

  #[inline]
  pub fn as_bool(&self) -> bool {
    self.kind.as_bool()
  }
}

#[derive(Clone, Debug)]
pub enum ValueKind {
  Unit,
  Int(i64),
  Float(f64),
  Bool(bool),
  Char(char),
  Str(SmolStr),
  Fn(Prototype, Block),
  Return(Box<Value>),
  Builtin(BuiltinFn),
  Array(Array),
}

impl ValueKind {
  #[inline]
  pub fn symbolize(&self) -> Symbol {
    todo!()
  }

  #[inline]
  pub fn as_bool(&self) -> bool {
    match self {
      Self::Bool(boolean) => *boolean,
      Self::Unit => false,
      _ => false,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Args(pub Vec<Arg>);

impl Args {
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

impl std::ops::Deref for Array {
  type Target = Vec<Value>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
