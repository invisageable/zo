use super::{Diagnostic, Error};

use crate::color;
use crate::report::{Report, ReportKind};

use swisskit::span::Span;

use ariadne::Fmt;
use smol_str::SmolStr;

/// The representation of evaluation errors.
#[derive(Debug)]
pub enum Eval {
  /// An invalid array access error message.
  InvalidArrayAcces(Span, SmolStr, SmolStr),
  /// An invalid tuple access error message.
  InvalidTupleAcces(Span, SmolStr, SmolStr),
  /// A mismatch arguments error message.
  MismatchArgument((Span, usize), (Span, usize)),
  /// A name clash error message.
  NameClash(NameClash),
  /// A not found error message.
  NotFound(NotFound),
  /// An out of bound error message.
  OutOfBound(OutOfBound),
  /// An out of bound error message.
  OutOfLoop(Span, SmolStr),
  /// An unknown binary operator error message.
  UnknownBinOp(Span, SmolStr),
  /// An unknown unary operator error message.
  UnknownUnOp(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for Eval {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::InvalidArrayAcces(span, indexed, index) => {
        todo!("{span} — {indexed} — {index}")
      }
      Self::InvalidTupleAcces(span, indexed, index) => {
        todo!("{span} — {indexed} — {index}")
      }
      Self::MismatchArgument(lhs, rhs) => todo!("{lhs:?} — {rhs:?}"),
      Self::NameClash(diagnostic) => diagnostic.report(),
      Self::NotFound(diagnostic) => diagnostic.report(),
      Self::OutOfBound(diagnostic) => diagnostic.report(),
      Self::OutOfLoop(span, behavior) => Report {
        kind: ReportKind::ERROR,
        message: format!(
          "{} {}",
          format_args!("`{}`", behavior.fg(color::hint())),
          "outside of the loop".fg(color::title())
        )
        .into(),
        labels: vec![(
          *span,
          format!("cannot `{behavior}` out of the loop").into(),
          color::error(),
        )],
        ..Default::default()
      },
      Self::UnknownBinOp(span, binop) => todo!("{span} — {binop}"),
      Self::UnknownUnOp(span, unop) => todo!("{span} — {unop}"),
    }
  }
}

#[derive(Debug)]
pub enum NameClash {
  /// A name clash error message for function.
  Fun(Span, SmolStr),
  /// A name clash error message for type.
  Ty(Span, SmolStr),
  /// A name clash error message for variable.
  Var(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for NameClash {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Fun(span, value) => todo!("{span} — {value}"),
      Self::Ty(span, value) => todo!("{span} — {value}"),
      Self::Var(span, value) => todo!("{span} — {value}"),
    }
  }
}

#[derive(Debug)]
pub enum NotFound {
  /// A not found error message for identifier.
  Ident(Span, SmolStr),
  /// A not found error message for function.
  Fun(Span, SmolStr),
  /// A not found error message for type.
  Ty(Span, SmolStr),
  /// A not found error message for variable.
  Var(Span, SmolStr),
}

impl<'a> Diagnostic<'a> for NotFound {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Ident(span, value) => todo!("{span} — {value}"),
      Self::Fun(span, value) => todo!("{span} — {value}"),
      Self::Ty(span, value) => todo!("{span} — {value}"),
      Self::Var(span, value) => todo!("{span} — {value}"),
    }
  }
}

#[derive(Debug)]
pub enum OutOfBound {
  /// An out of bound error message for array.
  Array(Span, i64),
  /// An out of bound error message for tuple.
  Tuple(Span, i64),
}

impl<'a> Diagnostic<'a> for OutOfBound {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::Array(span, size) => todo!("{span} — {size}"),
      Self::Tuple(span, size) => todo!("{span} — {size}"),
    }
  }
}

/// A break in while loop with value error message.
#[inline(always)]
pub fn break_in_while_loop_with_value(_span: Span) -> Error {
  todo!()
}

/// An invalid array access error message.
#[inline(always)]
pub fn invalid_array_access(
  span: Span,
  indexed: impl Into<SmolStr>,
  index: impl Into<SmolStr>,
) -> Error {
  Error::Eval(Eval::InvalidArrayAcces(span, indexed.into(), index.into()))
}

/// An invalid array access error message.
#[inline(always)]
pub fn invalid_tuple_access(
  span: Span,
  indexed: impl Into<SmolStr>,
  index: impl Into<SmolStr>,
) -> Error {
  Error::Eval(Eval::InvalidTupleAcces(span, indexed.into(), index.into()))
}

/// A mismatch arguments error message.
#[inline(always)]
pub fn mismatch_args(lhs: (Span, usize), rhs: (Span, usize)) -> Error {
  Error::Eval(Eval::MismatchArgument(lhs, rhs))
}

/// A name clash error message for function.
#[inline(always)]
pub fn name_clash_fun(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NameClash(NameClash::Fun(span, name.into())))
}

/// A name clash error message for type.
#[inline(always)]
pub fn name_clash_ty(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NameClash(NameClash::Ty(span, name.into())))
}

/// A name clash error message for variable.
#[inline(always)]
pub fn name_clash_var(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NameClash(NameClash::Var(span, name.into())))
}

/// A not found error message for function.
#[inline(always)]
pub fn not_found_fun(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Fun(span, name.into())))
}

/// A not found error message for function.
#[inline(always)]
pub fn not_found_ident(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Ident(span, name.into())))
}

/// A not found error message for type.
#[inline(always)]
pub fn not_found_ty(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Ty(span, name.into())))
}

/// A not found error message for variable.
#[inline(always)]
pub fn not_found_var(span: Span, name: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::NotFound(NotFound::Var(span, name.into())))
}

/// A out of bound error message for a array.
#[inline(always)]
pub fn out_of_bound_array(span: Span, index: i64) -> Error {
  Error::Eval(Eval::OutOfBound(OutOfBound::Array(span, index)))
}

/// A out of bound error message for a tuple.
#[inline(always)]
pub fn out_of_bound_tuple(span: Span, index: i64) -> Error {
  Error::Eval(Eval::OutOfBound(OutOfBound::Tuple(span, index)))
}

/// A out of loop error message.
#[inline(always)]
pub fn out_of_loop(span: Span, behavior: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::OutOfLoop(span, behavior.into()))
}

/// The unknown binary operator error.
#[inline(always)]
pub fn unknown_binop(span: Span, binop: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::UnknownUnOp(span, binop.into()))
}

/// The unknown unary operator error.
#[inline(always)]
pub fn unknown_unop(span: Span, unop: impl Into<SmolStr>) -> Error {
  Error::Eval(Eval::UnknownUnOp(span, unop.into()))
}
