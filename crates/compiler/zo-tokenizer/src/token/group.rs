/// The representation of group tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Group {
  /// An open parenthesis.
  ParenOpen,
  /// A close parenthesis.
  ParenClose,
  /// An open brace.
  BraceOpen,
  /// A close brace.
  BraceClose,
  /// An open bracket.
  BracketOpen,
  /// A close bracket.
  BracketClose,
}

impl From<char> for Group {
  fn from(group: char) -> Self {
    match group as u8 {
      b'(' => Self::ParenOpen,
      b')' => Self::ParenClose,
      b'{' => Self::BraceOpen,
      b'}' => Self::BraceClose,
      b'[' => Self::BracketOpen,
      b']' => Self::BracketClose,
      _ => unreachable!("{group}"),
    }
  }
}

impl std::fmt::Display for Group {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::ParenOpen => write!(f, "("),
      Self::ParenClose => write!(f, ")"),
      Self::BraceOpen => write!(f, "{{"),
      Self::BraceClose => write!(f, "}}"),
      Self::BracketOpen => write!(f, "["),
      Self::BracketClose => write!(f, "]"),
    }
  }
}
