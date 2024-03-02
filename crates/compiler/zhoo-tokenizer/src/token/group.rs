//! ...

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Group {
  ParenOpen,
  ParenClose,
  BraceOpen,
  BraceClose,
  BracketOpen,
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
