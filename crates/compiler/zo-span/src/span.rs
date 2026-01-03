use serde::Serialize;

/// The representation a region of source code by its start and end byte
/// offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Span {
  /// The starting position.
  pub start: u32,
  /// The length of the span.
  pub len: u16,
}
impl Span {
  /// A span initialize at zero.
  pub const ZERO: Self = Self::new(0, 0);

  /// Creates a new [`Span`] instance.
  ///
  /// #### examples.
  ///
  /// ```ignore
  /// use swisskit::span::Span;
  ///
  /// let spn = Span::of(0, 3);
  ///
  /// assert_eq!(spn.start, 0);
  /// assert_eq!(spn.end(), 3);
  /// ```
  #[inline(always)]
  pub const fn new(start: u32, len: u16) -> Self {
    Self { start, len }
  }

  /// Returns the end position of the span (exclusive).
  #[inline(always)]
  pub const fn end(&self) -> u32 {
    self.start + self.len as u32
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
    write!(f, "{}..{}", self.start, self.end())
  }
}
