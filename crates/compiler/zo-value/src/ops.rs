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

use super::value::{Value, ValueKind};

use swisskit::span::Span;

impl std::ops::Add for Value {
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
      _ => unreachable!(),
    }
  }
}

impl std::ops::Sub for Value {
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

impl std::ops::Mul for Value {
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

impl std::ops::Div for Value {
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

impl std::ops::Rem for Value {
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

impl std::ops::BitAnd for Value {
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

impl std::ops::BitOr for Value {
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

impl std::ops::BitXor for Value {
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

impl std::ops::Shl for Value {
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

impl std::ops::Shr for Value {
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
