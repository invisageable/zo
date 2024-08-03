use swisskit::span::Span;

// note #1 — I am questionning myself about how to deal with span in operator
// context. For example:
//
// 40 + 100
// -- - ---
// |  |  |
// 2  1  3
//
// In this instruction the left-hand side as his length span, the binary
// operator too and the right-hand side also. But when the interpreter will
// evaluate that the result will be:
//
// 140
// ---
//  |
//  3
//
// Now after the computation, the span is different. At this moment I do not
// know how to deal with that. First tought is to used a zero span or to not
// integrate the notion of span for `Value`.
//
// I really don't know what to do. Maybe this is trivial. Because errors must be
// handle before this kind of evaluation. So in this case we don't care about
// span.

/// The representation of a value.
#[derive(Clone, Copy, Debug)]
pub struct Value {
  /// The kind value.
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
}

/// The representation of a kind value.
#[derive(Clone, Copy, Debug)]
pub enum ValueKind {
  /// A unit value — `'()'`.
  Unit,
  /// A unit value — `'0'`, `'42'`.
  Int(i64),
}

impl std::ops::Add for &Value {
  type Output = Value;

  ///Performs the `+` operation.
  #[inline]
  fn add(self, rhs: Self) -> Self::Output {
    match (&self.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        Value::int(lhs + rhs, Span::ZERO) // note #1.
      }
      _ => unreachable!(),
    }
  }
}
