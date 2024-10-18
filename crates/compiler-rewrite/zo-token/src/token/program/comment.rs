/// The representation of a comment token kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Comment {
  /// A line comment.
  Line,
  /// A line doc comment.
  LineDoc(String),
}

impl std::fmt::Display for Comment {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Line => write!(f, "line-comment"),
      Self::LineDoc(_) => write!(f, "line-doc-comment"),
    }
  }
}
