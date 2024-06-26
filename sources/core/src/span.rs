//! ...

pub trait AsSpan {
  fn as_span(&self) -> Span;
}

/// The region of source code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
  pub lo: usize,
  pub hi: usize,
}

impl Span {
  pub const ZERO: Self = Self::of(0usize, 0usize);

  #[inline]
  pub const fn of(lo: usize, hi: usize) -> Self {
    Self { lo, hi }
  }

  #[inline]
  pub fn merge(a: Span, b: Span) -> Self {
    let lo = std::cmp::min(a.lo, b.lo);
    let hi = std::cmp::max(a.hi, b.hi);

    Self::of(lo, hi)
  }

  #[inline]
  pub fn to(&self, span: Span) -> Self {
    Self::merge(*self, span)
  }
}

impl Default for Span {
  fn default() -> Self {
    Self::ZERO
  }
}

impl std::fmt::Display for Span {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "({}..{})", self.lo, self.hi)
  }
}

impl From<Span> for std::ops::Range<usize> {
  fn from(span: Span) -> Self {
    span.lo..span.hi
  }
}

#[cfg(test)]
mod test {
  use super::Span;

  #[test]
  fn should_make_span_zero() {
    let span = Span::ZERO;

    assert!(span.lo == 0usize);
    assert!(span.hi == 0usize);
  }

  #[test]
  fn should_make_span() {
    let span = Span::of(0usize, 14usize);

    assert!(span.lo == 0usize);
    assert!(span.hi == 14usize);
  }

  #[test]
  fn should_make_span_from_another_span() {
    let from = Span::of(0usize, 14usize);
    let span = Span::from(from);

    assert!(span.lo == 0usize);
    assert!(span.hi == 14usize);
  }

  #[test]
  fn should_merge_spans() {
    let lo = Span::of(0usize, 14usize);
    let hi = Span::of(16usize, 20usize);
    let span = Span::merge(lo, hi);

    assert!(span.lo == 0usize);
    assert!(span.hi == 20usize);
  }
}
