/// The representation of a comment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Comment {
  /// A line comment — handled in [`TokenizerMode::Program`].
  Line,
  /// A line doc comment — handled in [`TokenizerMode::Program`].
  LineDoc,
  /// A line html comment — handled in [`TokenizerMode::Template`].
  LineHtml,
}

impl std::fmt::Display for Comment {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Line => write!(f, "line-comment"),
      Self::LineDoc => write!(f, "line-doc-comment"),
      Self::LineHtml => write!(f, "line-html-comment"),
    }
  }
}
