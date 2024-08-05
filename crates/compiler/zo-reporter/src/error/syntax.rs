use super::{Diagnostic, Error};

use crate::report::Report;

use smol_str::SmolStr;
use swisskit::span::Span;

/// The representation of syntax analysis errors.
#[derive(Debug)]
pub enum Syntax {
  /// The expected binary operator error.
  ExpectedBinOp(Span, SmolStr),
  /// The expected boolean error.
  ExpectedBool(Span, SmolStr),
  /// The expected float error.
  ExpectedFloat(Span, SmolStr),
  /// The expected identifier error.
  ExpectedIdent(Span, SmolStr),
  /// The expected global variable error.
  ExpectedGlobalVar(Span, SmolStr),
  /// The expected integer error.
  ExpectedInt(Span, SmolStr),
  /// The expected local variable error.
  ExpectedLocalVar(Span, SmolStr),
  /// The expected type error.
  ExpectedTy(Span, SmolStr),
  /// The expected unary operator error.
  ExpectedUnOp(Span, SmolStr),
  /// The invalid infix error.
  InvalidInfix(Span, SmolStr),
  /// The invalid prefix error.
  InvalidPrefix(Span, SmolStr),
  /// The unexpected token error.
  UnexpectedToken(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for Syntax {
  #[inline]
  fn report(&self) -> Report<'a> {
    todo!()
  }
}

/// The expected binary operator error.
#[inline]
pub fn expected_binop(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedBinOp(span, token.into()))
}

/// The expected boolean literal error.
#[inline]
pub fn expected_bool(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedBool(span, token.into()))
}

/// The expected float literal error.
#[inline]
pub fn expected_float(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedFloat(span, token.into()))
}

/// The expected global variable error.
#[inline]
pub fn expected_global_var(span: Span, var: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedGlobalVar(span, var.into()))
}

/// The expected identifier error.
#[inline]
pub fn expected_ident(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedIdent(span, token.into()))
}

/// The expected  error.
#[inline]
pub fn expected_int(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedInt(span, token.into()))
}

/// The expected local variable error.
#[inline]
pub fn expected_local_var(span: Span, var: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedLocalVar(span, var.into()))
}

/// The expected type error.
#[inline]
pub fn expected_ty(span: Span, ty: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedTy(span, ty.into()))
}

/// The expected unary operator error.
#[inline]
pub fn expected_unop(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::ExpectedUnOp(span, token.into()))
}

/// The invalid infix error.
#[inline]
pub fn invalid_infix(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::InvalidInfix(span, token.into()))
}

/// The invalid prefix error.
#[inline]
pub fn invalid_prefix(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::InvalidPrefix(span, token.into()))
}

/// The unexpected token error.
#[inline]
pub fn unexpected_token(span: Span, token: impl Into<SmolStr>) -> Error {
  Error::Syntax(Syntax::UnexpectedToken(span, token.into()))
}
