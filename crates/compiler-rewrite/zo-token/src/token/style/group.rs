/// The representation of group tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Group {
  /// An open parenthesis group — `(`.
  ParenOpen,
  /// A close parenthesis group — `)`.
  ParenClose,
  /// An open brace group — `{`.
  BraceOpen,
  /// A close brace group — `}`.
  BraceClose,
  /// An open bracket group — `[`.
  BracketOpen,
  /// A close bracket group — `]`.
  BracketClose,
}

impl From<char> for Group {
  fn from(ch: char) -> Self {
    match ch {
      '(' => Self::ParenOpen,
      ')' => Self::ParenClose,
      '{' => Self::BraceOpen,
      '}' => Self::BraceClose,
      '[' => Self::BracketOpen,
      ']' => Self::BracketClose,
      _ => unreachable!("{ch}"),
    }
  }
}

impl std::fmt::Display for Group {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
