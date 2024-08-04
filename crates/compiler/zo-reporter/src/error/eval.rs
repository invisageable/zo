use super::{Diagnostic, Error};

use crate::report::Report;

use swisskit::span::Span;

use smol_str::SmolStr;

/// The representation of evaluation errors.
#[derive(Debug)]
pub enum Eval {
  /// An unknown binary operator error.
  UnknownBinOp(Span, SmolStr),
  /// An unknown unary operator error.
  UnknownUnOp(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for Eval {
  #[inline]
  fn report(&self) -> Report<'a> {
    todo!()
  }
}

/// The unknown binary operator error.
#[inline]
pub const fn unknown_binop(span: Span, unop: SmolStr) -> Error {
  Error::Eval(Eval::UnknownUnOp(span, unop))
}

/// The unknown unary operator error.
#[inline]
pub const fn unknown_unop(span: Span, unop: SmolStr) -> Error {
  Error::Eval(Eval::UnknownUnOp(span, unop))
}
