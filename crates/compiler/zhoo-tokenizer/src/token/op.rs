use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum Op {
  Equal,
  Plus,
  Minus,
  Asterisk,
  Slash,
  Percent,
  Circumflex,
  Exclamation,
  ColonEqual,
}

impl From<char> for Op {
  fn from(op: char) -> Self {
    match op as u8 {
      b'=' => Self::Equal,
      b'+' => Self::Plus,
      b'-' => Self::Minus,
      b'*' => Self::Asterisk,
      b'/' => Self::Slash,
      b'%' => Self::Percent,
      b'^' => Self::Circumflex,
      b'!' => Self::Exclamation,
      _ => unreachable!("{op}"),
    }
  }
}

impl From<&str> for Op {
  fn from(op: &str) -> Self {
    match op {
      ":=" => Self::ColonEqual,
      _ => unreachable!("{op}"),
    }
  }
}

impl std::fmt::Display for Op {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Equal => write!(f, "="),
      Self::Plus => write!(f, "+"),
      Self::Minus => write!(f, "-"),
      Self::Asterisk => write!(f, "*"),
      Self::Slash => write!(f, "/"),
      Self::Percent => write!(f, "%"),
      Self::Circumflex => write!(f, "^"),
      Self::Exclamation => write!(f, "!"),
      Self::ColonEqual => write!(f, ":="),
    }
  }
}
