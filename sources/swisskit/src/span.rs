pub mod source;

/// The behavoir for span.
pub trait AsSpan {
  /// Converts an instance into span.
  fn as_span(&self) -> Span;
}

/// The representation of a span within a source file.
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
  /// A starting position.
  pub lo: usize,
  /// A ending position.
  pub hi: usize,
}

impl Span {
  /// A zero span.
  pub const ZERO: Self = Self::of(0usize, 0usize);

  /// Create a new span from an union between two spans.
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
  #[inline(always)]
  pub const fn of(lo: usize, hi: usize) -> Self {
    assert!(hi >= lo);

    Self { lo, hi }
  }

  /// Combines two spans.
  #[inline(always)]
  pub fn merge(self, rhs: Span) -> Self {
    let lo = std::cmp::min(self.lo, rhs.lo);
    let hi = std::cmp::max(self.hi, rhs.hi);

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
  #[inline(always)]
  pub const fn len(&self) -> usize {
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
  #[inline(always)]
  pub const fn is_empty(&self) -> bool {
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
  #[inline(always)]
  pub const fn contains(&self, pos: usize) -> bool {
    self.lo <= pos && pos < self.hi
  }
}

impl Default for Span {
  /// Creates a default Span — default values are sets to zero.
  #[inline(always)]
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
  #[inline(always)]
  fn from(span: Span) -> Self {
    span.lo as usize..span.hi as usize
  }
}
