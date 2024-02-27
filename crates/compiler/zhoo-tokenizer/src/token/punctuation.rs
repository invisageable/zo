use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum Punctuation {
  Comma,
  Period,
  Colon,
  ColonColon,
  Semicolon,
  MinusGreaterThan,
}

impl From<char> for Punctuation {
  fn from(punctuation: char) -> Self {
    match punctuation as u8 {
      b',' => Self::Comma,
      b'.' => Self::Period,
      b':' => Self::Colon,
      b';' => Self::Semicolon,
      _ => unreachable!("{punctuation}"),
    }
  }
}

impl From<&str> for Punctuation {
  fn from(punctuation: &str) -> Self {
    match punctuation {
      "::" => Self::ColonColon,
      "->" => Self::MinusGreaterThan,
      _ => unreachable!("{punctuation}"),
    }
  }
}

impl std::fmt::Display for Punctuation {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Comma => write!(f, ","),
      Self::Period => write!(f, "."),
      Self::Colon => write!(f, ":"),
      Self::ColonColon => write!(f, "::"),
      Self::Semicolon => write!(f, ";"),
      Self::MinusGreaterThan => write!(f, "->"),
    }
  }
}
