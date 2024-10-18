/// The representation of a comment delimiter token kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Comment {
  /// A comment delimiter open token kind.
  Open,
  /// A comment delimiter close token kind.
  Close,
}

impl std::fmt::Display for Comment {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Open => write!(f, "<!--"),
      Self::Close => write!(f, "-->"),
    }
  }
}
