use super::{Diagnostic, Error};

use crate::report::Report;

use smol_str::SmolStr;
use swisskit::span::Span;

/// The representation of syntax analysis errors.
#[derive(Debug)]
pub enum Syntax {
  ExpectedInt(Span, SmolStr),
  InvalidInfix(Span, SmolStr),
  InvalidPrefix(Span, SmolStr),
  UnexpectedToken(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for Syntax {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}

/// The expected integer literal error.
#[inline]
pub const fn expected_int(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::ExpectedInt(span, token))
}

/// The invalid infix error.
#[inline]
pub const fn invalid_infix(span: Span, op: SmolStr) -> Error {
  Error::Syntax(Syntax::InvalidInfix(span, op))
}

/// The invalid prefix error.
#[inline]
pub const fn invalid_prefix(span: Span, op: SmolStr) -> Error {
  Error::Syntax(Syntax::InvalidPrefix(span, op))
}

/// The unexpected token error.
#[inline]
pub const fn unexpected_token(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::UnexpectedToken(span, token))
}
