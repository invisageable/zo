/// The representation of delimiter tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delim {
  /// An dot delimiter — `.`.
  Dot,
  /// A comma delimiter — `,`.
  Comma,
}

impl From<char> for Delim {
  fn from(ch: char) -> Self {
    match ch {
      '.' => Self::Dot,
      ',' => Self::Comma,
      _ => unreachable!("{ch}"),
    }
  }
}

impl std::fmt::Display for Delim {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Dot => write!(f, "."),
      Self::Comma => write!(f, ","),
    }
  }
}
