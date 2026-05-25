use zo_span::Span;

use tower_lsp::lsp_types::{Position, Range};

/// Precomputed line-start byte offsets for O(1) line lookup.
pub struct LineIndex {
  line_starts: Vec<u32>,
}

impl LineIndex {
  /// Build from source text. O(n) scan, done once per file.
  pub fn new(source: &str) -> Self {
    let mut line_starts = vec![0u32];

    for (i, byte) in source.bytes().enumerate() {
      if byte == b'\n' {
        line_starts.push((i + 1) as u32);
      }
    }

    Self { line_starts }
  }

  /// LSP Position (line, character) to byte offset.
  pub fn offset(&self, line: u32, col: u32) -> u32 {
    let start = self.line_starts.get(line as usize).copied().unwrap_or(0);

    start + col
  }

  /// Byte offset to LSP Position.
  pub fn position(&self, offset: u32) -> Position {
    let line = self
      .line_starts
      .partition_point(|&s| s <= offset)
      .saturating_sub(1);

    let col = offset - self.line_starts[line];

    Position::new(line as u32, col)
  }

  /// Span to LSP Range.
  pub fn range(&self, span: Span) -> Range {
    Range::new(self.position(span.start), self.position(span.end()))
  }
}
