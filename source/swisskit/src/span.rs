pub mod source;

/// The behavoir for span.
pub trait AsSpan {
  /// Converts an instance into span.
  fn as_span(&self) -> Span;
}

/// A region in a source code.
///
/// #### understanding.
///
/// ```txt
/// Index:  0 1 2 3 4 5 6 7 8 9 10
///         i m u   x   =   4 2  ;
/// Span:   [---]   [ ]   [ ]   [--]
/// lo-hi:  0-3     4-5   6-7   8-10  10-11
/// ```
///
/// The span for `ìmu` starts at index 0 (lo) and ends at index 3 (hi).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
  /// The starting point of the [`Span`].
  pub lo: usize,
  /// The ending point of the [`Span`].
  pub hi: usize,
}

impl Span {
  /// A constant span, used as a placeholder.
  pub const ZERO: Self = Self::of(0, 0);

  /// Creates a [`Span`] instance from a specific location.
  ///
  /// #### examples.
  ///
  /// ```
  /// use swisskit::span::Span;
  ///
  /// let spn = Span::of(0, 3);
  ///
  /// assert_eq!(spn.lo, 0);
  /// assert_eq!(spn.hi, 3);
  /// ```
  #[inline]
  pub const fn of(lo: usize, hi: usize) -> Self {
    Self { lo, hi }
  }

  /// Crates a [`Span`] instance from an union between two spans.
  ///
  /// #### examples.
  ///
  /// ```
  /// use swisskit::span::Span;
  ///
  /// let lhs = Span::of(5, 10);
  /// let rhs = Span::of(20, 24);
  /// let spn = Span::merge(lhs, rhs);
  ///
  /// assert_eq!(spn.lo, 5);
  /// assert_eq!(spn.hi, 24);
  /// ```
  #[inline]
  pub fn merge(a: Span, b: Span) -> Self {
    let lo = std::cmp::min(a.lo, b.lo);
    let hi = std::cmp::max(a.hi, b.hi);

    Self::of(lo, hi)
  }

  /// Gets the length of the span.
  ///
  /// #### examples.
  ///
  /// ```
  /// use swisskit::span::Span;
  ///
  /// let spn = Span::of(5, 10);
  ///
  /// assert_eq!(spn.len(), 5);
  /// ```
  #[inline]
  pub fn len(&self) -> usize {
    self.hi - self.lo
  }

  /// Check if the span is empty.
  ///
  /// #### examples.
  ///
  /// ```
  /// use swisskit::span::Span;
  ///
  /// let spn = Span::of(0, 0);
  ///
  /// assert!(spn.is_empty());
  /// ```
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.lo == self.hi
  }

  /// Checks if a position is within the span.
  ///
  /// #### examples.
  ///
  /// ```
  /// use swisskit::span::Span;
  ///
  /// let spn = Span::of(5, 10);
  ///
  /// assert_eq!(spn.contains(7), true);
  /// assert_eq!(spn.contains(usize::MAX), false);
  /// ```
  #[inline]
  pub fn contains(&self, pos: usize) -> bool {
    self.lo <= pos && pos < self.hi
  }
}

impl Default for Span {
  #[inline]
  fn default() -> Self {
    Self::ZERO
  }
}

impl std::fmt::Display for Span {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}..{}", self.lo, self.hi)
  }
}

impl From<Span> for std::ops::Range<usize> {
  #[inline]
  fn from(span: Span) -> Self {
    span.lo..span.hi
  }
}
