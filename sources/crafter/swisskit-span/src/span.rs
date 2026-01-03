use serde::{Deserialize, Serialize};

/// Represents a continuous segment or region within a source text,
/// identified by absolute character positions as well as line and column
/// numbers. Useful for tracking spans in source code parsing, error reporting,
/// or syntax highlighting.
#[derive(
  Debug, Default, Clone, Copy, Eq, PartialEq, Hash, Deserialize, Serialize,
)]
pub struct Span {
  /// The zero-based absolute start position (inclusive) of the span in the
  /// source.
  pub start: usize,

  /// The zero-based absolute end position (exclusive) of the span in the
  /// source.
  pub end: usize,

  /// The one-based line number where the span starts.
  pub start_line: usize,

  /// The one-based line number where the span ends.
  pub end_line: usize,

  /// The one-based column number where the span starts.
  pub start_col: usize,

  /// The one-based column number where the span ends.
  pub end_col: usize,
}

impl Span {
  /// A zero-length span positioned at the start of the source (position 0, line
  /// 0, column 0).
  ///
  /// ## example.
  ///
  /// ```
  /// use swisskit_span::span::Span;
  ///
  /// let span = Span::ZERO;
  ///
  /// assert_eq!(span.start, 0);
  /// assert_eq!(span.end, 0);
  /// ```
  pub const ZERO: Self = Self::of(0, 0, 0, 0, 0, 0);

  /// Creates a new [`Span`] from the given start and end positions along with
  /// line and column boundaries.
  ///
  /// ## example.
  ///
  /// ```
  /// use swisskit_span::span::Span;
  ///
  /// let span = Span::of(5, 10, 1, 1, 6, 11);
  ///
  /// assert_eq!(span.start, 5);
  /// assert_eq!(span.end, 10);
  /// assert_eq!(span.start_line, 1);
  /// assert_eq!(span.end_line, 1);
  /// assert_eq!(span.start_col, 6);
  /// assert_eq!(span.end_col, 11);
  /// ```
  ///
  /// ## panics.
  ///
  /// Panics if `end < start`, since a span cannot have a negative length.
  ///
  /// ## parameters.
  ///
  /// - `start` — Zero-based inclusive start byte offset in the source.
  /// - `end` — Zero-based exclusive end byte offset in the source.
  /// - `start_line` — One-based starting line number.
  /// - `end_line` — One-based ending line number.
  /// - `start_col` — One-based starting column number.
  /// - `end_col` — One-based ending column number.
  ///
  /// ## returns.
  ///
  /// A new [`Span`] instance representing the region defined by these
  /// boundaries.
  pub const fn of(
    start: usize,
    end: usize,
    start_line: usize,
    end_line: usize,
    start_col: usize,
    end_col: usize,
  ) -> Self {
    assert!(
      end >= start,
      "Span end position must not be less than start position.",
    );

    Self {
      start,
      end,
      start_line,
      end_line,
      start_col,
      end_col,
    }
  }

  /// Returns a new [`Span`] that covers the union of `self` and `rhs`,
  /// spanning from the earliest start position to the latest end position.
  ///
  /// This is useful for combining adjacent or overlapping spans into a single
  /// span.
  ///
  /// # example.
  ///
  /// ```
  /// use my_crate::Span;
  ///
  /// let s1 = Span::of(5, 10, 1, 1, 6, 11);
  /// let s2 = Span::of(8, 15, 1, 2, 9, 5);
  /// let merged = s1.merge(s2);
  ///
  /// assert_eq!(merged.start, 5);
  /// assert_eq!(merged.end, 15);
  /// assert_eq!(merged.start_line, 1);
  /// assert_eq!(merged.end_line, 2);
  /// assert_eq!(merged.start_col, 6);
  /// assert_eq!(merged.end_col, 11);
  /// ```
  ///
  /// # parameters.
  ///
  /// - `rhs` — Another `Span` to merge with.
  ///
  /// # Returns
  ///
  /// A new `Span` covering the full extent of both spans.
  pub fn merge(self, rhs: Span) -> Self {
    Self::of(
      std::cmp::min(self.start, rhs.start),
      std::cmp::max(self.end, rhs.end),
      std::cmp::min(self.start_line, rhs.start_line),
      std::cmp::max(self.end_line, rhs.end_line),
      std::cmp::min(self.start_col, rhs.start_col),
      std::cmp::max(self.end_col, rhs.end_col),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::Span;

  #[test]
  fn makes_zero_span() {
    let span = Span::ZERO;

    assert_eq!(span.start, 0);
    assert_eq!(span.end, 0);
    assert_eq!(span.start_line, 0);
    assert_eq!(span.end_line, 0);
    assert_eq!(span.start_col, 0);
    assert_eq!(span.end_col, 0);
  }

  #[test]
  fn makes_span() {
    let span = Span::of(1, 5, 1, 1, 2, 6);

    assert_eq!(span.start, 1);
    assert_eq!(span.end, 5);
    assert_eq!(span.start_line, 1);
    assert_eq!(span.end_line, 1);
    assert_eq!(span.start_col, 2);
    assert_eq!(span.end_col, 6);
  }

  #[test]
  #[should_panic(
    expected = "Span end position must not be less than start position."
  )]
  fn makes_span_panic() {
    let _ = Span::of(10, 5, 1, 1, 2, 6);
  }

  #[test]
  fn merges_two_spans() {
    let s1 = Span::of(3, 8, 1, 1, 4, 9);
    let s2 = Span::of(6, 12, 1, 2, 7, 3);
    let merged = s1.merge(s2);

    assert_eq!(merged.start, 3);
    assert_eq!(merged.end, 12);
    assert_eq!(merged.start_line, 1);
    assert_eq!(merged.end_line, 2);
    assert_eq!(merged.start_col, 4);
    assert_eq!(merged.end_col, 9);
  }
}

#[cfg(test)]
mod tests_fuzz {
  use super::Span;

  use proptest::prelude::*;

  fn arb_span() -> impl Strategy<Value = Span> {
    (0usize..1_000)
      .prop_flat_map(|start| {
        (
          Just(start),
          start..start + 100,
          1usize..100,
          1usize..100,
          1usize..500,
          1usize..500,
        )
      })
      .prop_map(|(start, end, start_line, end_line, start_col, end_col)| {
        Span::of(start, end, start_line, end_line, start_col, end_col)
      })
  }

  proptest! {
    #[test]
    fn merge_spans_should_not_panic(s1 in arb_span(), s2 in arb_span()) {
      let _ = s1.merge(s2);
    }

    #[test]
    fn merged_span_contains_both_spans(s1 in arb_span(), s2 in arb_span()) {
      let merged = s1.merge(s2);

      assert!(merged.start <= s1.start && merged.start <= s2.start);
      assert!(merged.end >= s1.end && merged.end >= s2.end);
    }
  }
}
