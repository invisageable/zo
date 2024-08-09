use super::{Diagnostic, Error};

use crate::color;
use crate::report::Report;
use crate::report::ReportKind;

use swisskit::span::Span;

use ariadne::Fmt;
use smol_str::SmolStr;

/// The representation of semantic analysis errors.
#[derive(Debug)]
pub enum Semantic {
  /// A mismatched types error.
  MismatchedTy((Span, SmolStr), (Span, SmolStr)),
  /// A naming convention error.
  NamingConvention(Span, SmolStr, SmolStr),
}

impl<'a> Diagnostic<'a> for Semantic {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::MismatchedTy(t1, t2) => Report {
        kind: ReportKind::ERROR,
        message: format!("{}", "mismatched types".fg(color::title())).into(),
        labels: vec![
          (
            t1.0,
            format!("the left-hand side is {}", t1.1).into(),
            color::error(),
          ),
          (
            t2.0,
            format!("the right-hand side is {}", t2.1).into(),
            color::error(),
          ),
        ],
        ..Default::default()
      },
      Self::NamingConvention(_span, _name, _convention) => Report {
        kind: ReportKind::WARNING,
        ..Default::default()
      },
    }
  }
}

/// The mismatched types error.
///
/// * t1 refers to the left-hand side.
/// * t2 refers to the right-hand side.
#[inline]
pub const fn mismatched_types(
  t1: (Span, SmolStr),
  t2: (Span, SmolStr),
) -> Error {
  Error::Semantic(Semantic::MismatchedTy(t1, t2))
}

/// The naming conventino error.
#[inline]
pub fn naming_convention(
  span: Span,
  name: impl Into<SmolStr>,
  naming: impl Into<SmolStr>,
) -> Error {
  Error::Semantic(Semantic::NamingConvention(span, name.into(), naming.into()))
}
