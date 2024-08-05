use swisskit::span::Span;

use smol_str::{SmolStr, ToSmolStr};

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
  pub const fn array(array: Vec<Value>, span: Span) -> Self {
    Self::new(ValueKind::Array(array), span)
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
  Array(Vec<Value>),
}

impl ValueKind {
  /// Converts a value kind into a boolean.
  #[inline]
  pub fn as_bool(&self) -> bool {
    match self {
      Self::Bool(boolean) => *boolean,
      Self::Unit => false,
      _ => true,
    }
  }
}
