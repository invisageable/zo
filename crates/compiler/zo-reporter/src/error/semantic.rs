use super::{Diagnostic, Error};

use crate::report::Report;

use smol_str::SmolStr;
use swisskit::span::Span;

/// The representation of semantic analysis errors.
#[derive(Debug)]
pub enum Semantic {
  /// A mismatched types error.
  MismatchedTy((Span, SmolStr), (Span, SmolStr)),
}

impl<'a> Diagnostic<'a> for Semantic {
  #[inline]
  fn report(&self) -> Report<'a> {
    match self {
      Self::MismatchedTy(t1, t2) => todo!("{t1:?} — {t2:?}"),
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
