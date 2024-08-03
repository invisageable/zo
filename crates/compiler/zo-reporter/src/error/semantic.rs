use super::{Diagnostic, Error};

use crate::report::Report;

use smol_str::SmolStr;
use swisskit::span::Span;

/// The representation of semantic analysis errors.
#[derive(Debug)]
pub enum Semantic {
  MismatchedTy((Span, SmolStr), (Span, SmolStr)),
}

impl<'a> Diagnostic<'a> for Semantic {
  fn report(&self) -> Report<'a> {
    todo!()
  }
}

/// The expected integer literal error.
#[inline]
pub const fn mismatched_types(
  t1: (Span, SmolStr),
  t2: (Span, SmolStr),
) -> Error {
  Error::Semantic(Semantic::MismatchedTy(t1, t2))
}
