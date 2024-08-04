use super::{Diagnostic, Error};

use crate::report::Report;

use smol_str::SmolStr;
use swisskit::span::Span;

/// The representation of syntax analysis errors.
#[derive(Debug)]
pub enum Syntax {
  /// The expected boolean error.
  ExpectedBool(Span, SmolStr),
  /// The expected float error.
  ExpectedFloat(Span, SmolStr),
  /// The expected identifier error.
  ExpectedIdent(Span, SmolStr),
  /// The expected integer error.
  ExpectedInt(Span, SmolStr),
  /// The invalid infix error.
  InvalidInfix(Span, SmolStr),
  /// The invalid prefix error.
  InvalidPrefix(Span, SmolStr),
  /// The unexpected token error.
  UnexpectedToken(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for Syntax {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}

/// The expected boolean literal error.
#[inline]
pub const fn expected_bool(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::ExpectedBool(span, token))
}

/// The expected float literal error.
#[inline]
pub const fn expected_float(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::ExpectedFloat(span, token))
}

/// The expected identifier error.
#[inline]
pub const fn expected_ident(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::ExpectedIdent(span, token))
}

/// The expected integer literal error.
#[inline]
pub const fn expected_int(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::ExpectedInt(span, token))
}

/// The invalid infix error.
#[inline]
pub const fn invalid_infix(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::InvalidInfix(span, token))
}

/// The invalid prefix error.
#[inline]
pub const fn invalid_prefix(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::InvalidPrefix(span, token))
}

/// The unexpected token error.
#[inline]
pub const fn unexpected_token(span: Span, token: SmolStr) -> Error {
  Error::Syntax(Syntax::UnexpectedToken(span, token))
}
