/// The representation of a punctuation token kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Punctuation {
  /// A colon token kind.
  Colon,
  /// A semicolon token kind.
  Semicolon,
  /// A comma token kind.
  Comma,
}

impl From<&str> for Punctuation {
  fn from(punctuation: &str) -> Self {
    match punctuation {
      ":" => Self::Colon,
      ";" => Self::Semicolon,
      "," => Self::Comma,
      _ => unreachable!("{punctuation}"),
    }
  }
}

impl std::fmt::Display for Punctuation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Colon => write!(f, ":"),
      Self::Semicolon => write!(f, ";"),
      Self::Comma => write!(f, ","),
    }
  }
}
