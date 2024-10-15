use super::builtin::BuiltinFn;

use zo_ast::ast;

use swisskit::span::{AsSpan, Span};

use smol_str::{SmolStr, ToSmolStr};
use thin_vec::ThinVec;

/// The representation of a value.
#[derive(Clone, Debug)]
pub struct Value {
  /// The value kind.
  pub kind: ValueKind,
  /// The related span.
  pub span: Span,
}

impl Value {
  /// The zero value, it is used as a placeholder.
  pub const UNIT: Self = Self::new(ValueKind::Unit, Span::ZERO);

  /// Creates a new value.
  #[inline]
  pub const fn new(kind: ValueKind, span: Span) -> Self {
    Self { kind, span }
  }

  /// Creates a new unit.
  #[inline]
  pub fn unit(span: Span) -> Self {
    Self::new(ValueKind::Unit, span)
  }

  /// Creates a new integer value.
  #[inline]
  pub const fn int(int: i64, span: Span) -> Self {
    Self::new(ValueKind::Int(int), span)
  }

  /// Creates a new float value.
  #[inline]
  pub const fn float(float: f64, span: Span) -> Self {
    Self::new(ValueKind::Float(float), span)
  }

  /// Creates a new boolean value.
  #[inline]
  pub const fn bool(boolean: bool, span: Span) -> Self {
    Self::new(ValueKind::Bool(boolean), span)
  }

  /// Creates a new string value.
  #[inline]
  pub const fn str(string: String, span: Span) -> Self {
    Self::new(ValueKind::Str(string), span)
  }

  /// Creates a new array value.
  #[inline]
  pub const fn array(array: ThinVec<Value>, span: Span) -> Self {
    Self::new(ValueKind::Array(array), span)
  }

  /// Creates a new tuple value.
  #[inline]
  pub const fn tuple(tuple: ThinVec<Value>, span: Span) -> Self {
    Self::new(ValueKind::Tuple(tuple), span)
  }

  /// Creates a new return value.
  #[inline]
  pub fn ret(value: Value, span: Span) -> Self {
    Self::new(ValueKind::Return(Box::new(value)), span)
  }

  /// Creates a new stop value.
  #[inline]
  pub fn stop(value: Box<Value>, span: Span) -> Self {
    Self::new(ValueKind::Stop(value), span)
  }

  /// Creates a new skip value.
  #[inline]
  pub fn skip(span: Span) -> Self {
    Self::new(ValueKind::Skip, span)
  }

  /// Creates a new while.
  #[inline]
  pub fn loop_while(
    condition: Box<Value>,
    block: ast::Block,
    span: Span,
  ) -> Self {
    Self::new(ValueKind::While(condition, block), span)
  }

  /// Creates a new closure.
  #[inline]
  pub fn closure(
    prototype: ast::Prototype,
    block: ast::Block,
    span: Span,
  ) -> Self {
    Self::new(ValueKind::Closure(prototype, block), span)
  }

  /// Creates a new function.
  #[inline]
  pub fn fun(prototype: ast::Prototype, block: ast::Block, span: Span) -> Self {
    Self::new(ValueKind::Fun(prototype, block), span)
  }

  /// Converts a value into a boolean.
  #[inline]
  pub fn as_bool(&self) -> bool {
    self.kind.as_bool()
  }
}

impl From<Value> for SmolStr {
  #[inline]
  fn from(value: Value) -> Self {
    value.to_smolstr()
  }
}

/// The representation of a kind value.
#[derive(Clone, Debug)]
pub enum ValueKind {
  /// A unit value — `'()'`.
  Unit,
  /// A integer value — `'0'`, `'42'`.
  Int(i64),
  /// A floating-point value — `'0.5'`.
  Float(f64),
  /// bool — `false` or `true`.
  Bool(bool),
  /// A string — `"foo"` or `"bar oof rab"`.
  Str(String),
  /// array — `[1, 2, 3, 4]`.
  Array(ThinVec<Value>),
  /// tuple — `(1, 2, 3, 4)`.
  Tuple(ThinVec<Value>),
  /// loop instruction value — `loop {..}`.
  Loop(ast::Block),
  /// while instruction value — `while true {..}`.
  While(Box<Value>, ast::Block),
  /// return — `return foobar;`, `return;`.
  Return(Box<Value>),
  /// break — `break foobar;`, `break;`.
  Stop(Box<Value>),
  /// skip — `skip;`.
  Skip,
  /// closure — `fn (x) -> x`, `fn (x) {..}`.
  Closure(ast::Prototype, ast::Block),
  /// function — `fun foo() {}`.
  Fun(ast::Prototype, ast::Block),
  /// builtin function.
  Builtin(BuiltinFn),
}

impl ValueKind {
  /// Converts a value kind into a boolean.
  #[inline]
  pub fn as_bool(&self) -> bool {
    match self {
      Self::Unit => false,
      Self::Bool(boolean) => *boolean,
      _ => true,
    }
  }
}

/// The representation of arguments.
#[derive(Clone, Debug)]
pub struct Args(ThinVec<Value>);

impl Args {
  /// Creates a new arguments.
  #[inline]
  pub fn new(inputs: ThinVec<Value>) -> Self {
    Self(inputs)
  }
}
impl AsSpan for Args {
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

impl std::ops::Deref for Args {
  type Target = ThinVec<Value>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
