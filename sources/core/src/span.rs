use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Span {
  pub lo: usize,
  pub hi: usize,
}

impl Span {
  pub const ZERO: Self = Self::of(0usize, 0usize);

  pub const fn of(lo: usize, hi: usize) -> Self {
    Self { lo, hi }
  }

  pub fn merge(a: Span, b: Span) -> Span {
    let lo = std::cmp::min(a.lo, b.lo);
    let hi = std::cmp::max(a.hi, b.hi);

    Self::of(lo, hi)
  }
}

impl Default for Span {
  fn default() -> Self {
    Self::ZERO
  }
}

impl std::fmt::Display for Span {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}..{}", self.lo, self.hi)
  }
}

#[cfg(test)]
mod test {
  use super::Span;

  #[test]
  fn should_make_span_zero() {
    let span = Span::ZERO;

    assert!(span.lo == 0);
    assert!(span.hi == 0);
  }

  #[test]
  fn should_make_span() {
    let span = Span::of(0, 14);

    assert!(span.lo == 0);
    assert!(span.hi == 14);
  }

  #[test]
  fn should_make_span_from_another_span() {
    let from = Span::of(0, 14);
    let span = Span::from(from);

    assert!(span.lo == 0);
    assert!(span.hi == 14);
  }

  #[test]
  fn should_merge_spans() {
    let lo = Span::of(0, 14);
    let hi = Span::of(16, 20);
    let span = Span::merge(lo, hi);

    assert!(span.lo == 0);
    assert!(span.hi == 20);
  }
}
