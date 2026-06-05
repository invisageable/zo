//! Source geometry shared by the machine renderers.
//!
//! Byte-offset → 1-indexed line/col, inline snippet extraction,
//! source-file resolution, and fix-it span computation. Pure,
//! format-agnostic, deterministic — the JSON and XML renderers
//! both build on this single source of truth so a fix to the
//! line/col math can never land in one wire format and miss the
//! other.
//!
//! Line and column are **1-indexed** (matches every IDE,
//! `cargo build`, `tsc`, etc.). `col` counts UTF-8 chars (not
//! bytes), so `é` advances `col` by 1.

use crate::fixes::FixKind;

use zo_error::Error;

use std::path::PathBuf;

/// Resolves an error's source text and display filename.
pub(crate) fn file_for_error<'a>(
  error: &Error,
  files: &'a [(PathBuf, String)],
) -> (&'a str, String) {
  let idx = error
    .file_id()
    .map(|id| id as usize)
    .unwrap_or(0)
    .min(files.len().saturating_sub(1));

  let (path, source) = &files[idx];

  let filename = path
    .file_name()
    .map(|n| n.to_string_lossy().into_owned())
    .unwrap_or_else(|| path.to_string_lossy().into_owned());

  (source.as_str(), filename)
}

/// 1-indexed `(line, column)` for a byte position in
/// `source`. Counts `\n` for line and UTF-8 chars (not
/// bytes) on the current line for column — `é` advances
/// column by 1, matching every IDE's convention.
///
/// Clamps `byte_pos > source.len()` (the EOF-span case
/// when the file ends without a trailing newline). Returns
/// `(1, 1)` for an empty source or for `byte_pos == 0`.
pub(crate) fn line_col_for_byte(source: &str, byte_pos: usize) -> (u32, u32) {
  let pos = byte_pos.min(source.len());
  let mut line: u32 = 1;
  let mut col: u32 = 1;

  for (i, ch) in source.char_indices() {
    if i >= pos {
      break;
    }

    if ch == '\n' {
      line += 1;
      col = 1;
    } else {
      col += 1;
    }
  }

  (line, col)
}

/// 1-indexed `(line, col)` pair for two byte positions in a
/// single source walk. Equivalent to two `line_col_for_byte`
/// calls but iterates `char_indices` once when the second
/// position is ≥ the first — the common case for diagnostic
/// span endpoints. Falls back to the per-position helper
/// when `byte_end < byte_start`.
pub(crate) fn line_col_pair(
  source: &str,
  byte_start: usize,
  byte_end: usize,
) -> ((u32, u32), (u32, u32)) {
  if byte_end < byte_start {
    return (
      line_col_for_byte(source, byte_start),
      line_col_for_byte(source, byte_end),
    );
  }

  let start_pos = byte_start.min(source.len());
  let end_pos = byte_end.min(source.len());
  let mut start = (1u32, 1u32);
  let mut line: u32 = 1;
  let mut col: u32 = 1;
  let mut start_captured = false;

  for (i, ch) in source.char_indices() {
    if !start_captured && i >= start_pos {
      start = (line, col);
      start_captured = true;
    }
    if i >= end_pos {
      return (start, (line, col));
    }

    if ch == '\n' {
      line += 1;
      col = 1;
    } else {
      col += 1;
    }
  }

  if !start_captured {
    start = (line, col);
  }
  (start, (line, col))
}

/// Inline source context for a diagnostic. Returns
/// `(before, lines, after)`:
///
/// * `before` — up to `context` lines immediately preceding
///   the span's starting line.
/// * `lines` — every line the span touches (one entry for
///   single-line spans, more for multi-line spans).
/// * `after` — up to `context` lines immediately following
///   the span's ending line.
///
/// Lines are stored without trailing newlines. Saturates at
/// the source's bounds (when the span sits near the top or
/// bottom of the file, `before` / `after` shrink instead of
/// returning blank lines). Empty source returns three empty
/// vecs.
pub(crate) fn extract_snippet(
  source: &str,
  byte_start: usize,
  byte_end: usize,
  context: usize,
) -> (Vec<String>, Vec<String>, Vec<String>) {
  let lines: Vec<&str> = source.lines().collect();

  if lines.is_empty() {
    return (Vec::new(), Vec::new(), Vec::new());
  }

  // Single pass over the source's newlines determines both
  // line indices. Previous shape called `source[..b].matches`
  // twice — O(N) each — and re-scanned identical bytes for
  // every diagnostic. One walk now suffices for the common
  // `byte_start <= byte_end` case; the rare inverted case
  // falls back to two scans (correctness over micro-opt).
  let last = lines.len() - 1;
  let (start_line, end_line) =
    line_indices_for_range(source, byte_start, byte_end, last);

  let before_start = start_line.saturating_sub(context);
  let after_end = (end_line + 1 + context).min(lines.len());

  let before = lines[before_start..start_line]
    .iter()
    .map(|s| s.to_string())
    .collect();
  let span_lines = lines[start_line..=end_line]
    .iter()
    .map(|s| s.to_string())
    .collect();
  let after = lines[(end_line + 1).min(lines.len())..after_end]
    .iter()
    .map(|s| s.to_string())
    .collect();

  (before, span_lines, after)
}

/// 0-indexed line numbers for two byte positions, clamped to
/// `last_line`. Walks `source.bytes()` once when
/// `byte_end >= byte_start` (the diagnostic-span norm),
/// counting `\n` characters and snapshotting the start
/// position along the way.
fn line_indices_for_range(
  source: &str,
  byte_start: usize,
  byte_end: usize,
  last_line: usize,
) -> (usize, usize) {
  let len = source.len();
  let s_pos = byte_start.min(len);
  let e_pos = byte_end.min(len);

  if e_pos < s_pos {
    let count_nl = |b: usize| {
      source.as_bytes()[..b]
        .iter()
        .filter(|&&c| c == b'\n')
        .count()
    };
    return (
      count_nl(s_pos).min(last_line),
      count_nl(e_pos).min(last_line),
    );
  }

  let mut start_line = 0;
  let mut start_captured = false;
  let mut newlines = 0usize;

  for (i, &b) in source.as_bytes().iter().enumerate() {
    if !start_captured && i >= s_pos {
      start_line = newlines;
      start_captured = true;
    }
    if i >= e_pos {
      return (start_line.min(last_line), newlines.min(last_line));
    }
    if b == b'\n' {
      newlines += 1;
    }
  }

  if !start_captured {
    start_line = newlines;
  }
  (start_line.min(last_line), newlines.min(last_line))
}

/// The edit span a fix-it applies to, derived from its
/// `FixKind` relative to the diagnostic's primary span:
///
/// * `Insert` → zero-length point at `span_start`.
/// * `Replace` / `Delete` → the full `[span_start, span_end)`
///   range.
pub(crate) fn fix_span(
  kind: FixKind,
  span_start: u32,
  span_end: u32,
) -> (u32, u32) {
  match kind {
    FixKind::Insert => (span_start, span_start),
    FixKind::Replace | FixKind::Delete => (span_start, span_end),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_snippet_returns_context_lines() {
    let source = "line1\nline2\nline3\nline4\nline5\n";

    // Span on line 3 (byte 12..17), context = 1: expect
    // 1 line before, the span line, 1 line after.
    let (before, lines, after) = extract_snippet(source, 12, 17, 1);
    assert_eq!(before, vec!["line2"]);
    assert_eq!(lines, vec!["line3"]);
    assert_eq!(after, vec!["line4"]);

    // Context = 2 expands to 2 each.
    let (before, lines, after) = extract_snippet(source, 12, 17, 2);
    assert_eq!(before, vec!["line1", "line2"]);
    assert_eq!(lines, vec!["line3"]);
    assert_eq!(after, vec!["line4", "line5"]);

    // Span at start of file saturates `before` to empty.
    let (before, lines, _) = extract_snippet(source, 0, 5, 2);
    assert_eq!(before, Vec::<String>::new());
    assert_eq!(lines, vec!["line1"]);

    // Span at end of file saturates `after` to empty.
    let (_, lines, after) = extract_snippet(source, 24, 29, 2);
    assert_eq!(lines, vec!["line5"]);
    assert_eq!(after, Vec::<String>::new());

    // Multi-line span: `lines` contains every line touched.
    let (_, lines, _) = extract_snippet(source, 6, 17, 0);
    assert_eq!(lines, vec!["line2", "line3"]);

    // Empty source — all three vecs empty.
    let (b, l, a) = extract_snippet("", 0, 0, 2);
    assert!(b.is_empty() && l.is_empty() && a.is_empty());
  }

  #[test]
  fn line_col_for_byte_handles_multibyte_and_newlines() {
    // ASCII single-line: column counts chars, 1-indexed.
    assert_eq!(line_col_for_byte("hello", 0), (1, 1));
    assert_eq!(line_col_for_byte("hello", 3), (1, 4));
    assert_eq!(line_col_for_byte("hello", 5), (1, 6)); // EOF
    // `\n` resets column.
    assert_eq!(line_col_for_byte("a\nbc", 2), (2, 1));
    assert_eq!(line_col_for_byte("a\nbc", 4), (2, 3));
    // Multi-byte char advances column by 1, not by byte
    // length. `é` is 2 bytes (U+00E9); after `é` the
    // column is 2.
    assert_eq!(line_col_for_byte("é", 2), (1, 2));
    // `byte_pos` past end clamps to source.len().
    assert_eq!(line_col_for_byte("ab", 999), (1, 3));
    // Empty source.
    assert_eq!(line_col_for_byte("", 0), (1, 1));
    assert_eq!(line_col_for_byte("", 999), (1, 1));
  }

  #[test]
  fn fix_span_anchors_by_kind() {
    // Insert collapses to a zero-width point at the start.
    assert_eq!(fix_span(FixKind::Insert, 4, 11), (4, 4));
    // Replace / delete cover the whole primary span.
    assert_eq!(fix_span(FixKind::Replace, 4, 11), (4, 11));
    assert_eq!(fix_span(FixKind::Delete, 4, 11), (4, 11));
  }
}
