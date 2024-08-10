use super::builtin::BuiltinFn;

use zo_ast::ast;

use swisskit::span::Span;

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

  /// Creates a new break value.
  #[inline]
  pub fn brk(value: Box<Value>, span: Span) -> Self {
    Self::new(ValueKind::Break(value), span)
  }

  /// Creates a new continue value.
  #[inline]
  pub fn ctn(span: Span) -> Self {
    Self::new(ValueKind::Continue, span)
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
  Break(Box<Value>),
  /// continue — `continue;`.
  Continue,
  /// closure — `fn (x) -> x`, `fn (x) {..}`.
  Closure(ast::Prototype, ast::Block),
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
