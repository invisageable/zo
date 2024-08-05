use super::{Diagnostic, Error};

use crate::report::Report;

use swisskit::span::Span;

use smol_str::SmolStr;

/// The representation of evaluation errors.
#[derive(Debug)]
pub enum Eval {
  /// A invalid array access error message.
  InvalidArrayAcces(Span, SmolStr, SmolStr),
  /// A name clash error message.
  NameClash(NameClash),
  /// A not found error message.
  NotFound(NotFound),
  /// An unknown binary operator error message.
  UnknownBinOp(Span, SmolStr),
  /// An unknown unary operator error message.
  UnknownUnOp(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for Eval {
  #[inline]
  fn report(&self) -> Report<'a> {
    todo!()
  }
}

#[derive(Debug)]
enum NameClash {
  /// A name clash error message for function.
  Fun(Span, SmolStr),
  /// A name clash error message for type.
  Ty(Span, SmolStr),
  /// A name clash error message for variable.
  Var(Span, SmolStr),
}

#[derive(Debug)]
enum NotFound {
  /// A not found error message for element in a table.
  Array(Span, i64),
  /// A not found error message for identifier.
  Ident(Span, SmolStr),
  /// A not found error message for function.
  Fun(Span, SmolStr),
  /// A not found error message for type.
  Ty(Span, SmolStr),
  /// A not found error message for variable.
  Var(Span, SmolStr),
}

/// A name clash error message for function.
#[inline]
pub fn invalid_array_access(
  span: Span,
  indexed: impl Into<SmolStr>,
  index: impl Into<SmolStr>,
) -> Error {
  Error::Eval(Eval::InvalidArrayAcces(span, indexed.into(), index.into()))
}

/// A name clash error message for function.
#[inline]
pub fn name_clash_fun(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NameClash(NameClash::Fun(span, name.into())))
}

/// A name clash error message for type.
#[inline]
pub fn name_clash_ty(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NameClash(NameClash::Ty(span, name.into())))
}

/// A name clash error message for variable.
#[inline]
pub fn name_clash_var(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NameClash(NameClash::Var(span, name.into())))
}

/// A not found error message for element in a table.
#[inline]
pub fn not_found_array_elmt(span: Span, index: i64) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Array(span, index)))
}

/// A not found error message for function.
#[inline]
pub fn not_found_fun(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Fun(span, name.into())))
}

/// A not found error message for function.
#[inline]
pub fn not_found_ident(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Ident(span, name.into())))
}

/// A not found error message for type.
#[inline]
pub fn not_found_ty(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Ty(span, name.into())))
}

/// A not found error message for variable.
#[inline]
pub fn not_found_var(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Var(span, name.into())))
}

/// The unknown binary operator error.
#[inline]
pub fn unknown_binop(span: Span, binop: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::UnknownUnOp(span, binop.into()))
}

/// The unknown unary operator error.
#[inline]
pub fn unknown_unop(span: Span, unop: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::UnknownUnOp(span, unop.into()))
}
