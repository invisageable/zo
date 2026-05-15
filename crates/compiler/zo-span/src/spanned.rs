use crate::span::Span;

/// A value paired with its source span. Generic so every
/// "thing + where it came from" combo across the compiler
/// (load paths, link entries, future diagnostic carriers)
/// shares one shape and one set of accessor helpers.
#[derive(Clone, Debug, PartialEq)]
pub struct Spanned<T> {
  pub value: T,
  pub span: Span,
}

impl<T> Spanned<T> {
  /// Wrap a `value` with its `span`.
  #[inline]
  pub fn new(value: T, span: Span) -> Self {
    Self { value, span }
  }
}

impl<T: Copy> Copy for Spanned<T> {}
