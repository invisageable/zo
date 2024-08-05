/// The representation of group tokens.
///
/// A group is a [`super::TokenKind`] used as a delimiter as a separation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Group {
  /// An open parenthesis i.e `(`.
  ParenOpen,
  /// A close parenthesis i.e `)`.
  ParenClose,
  /// An open brace i.e `{`.
  BraceOpen,
  /// A close brace i.e `}`.
  BraceClose,
  /// An open bracket i.e `[`.
  BracketOpen,
  /// A close bracket i.e `]`.
  BracketClose,
}

impl From<char> for Group {
  #[inline]
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
