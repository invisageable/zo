/// The representation of an integer base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Base {
  /// A binary base — `0b`.
  Bin = 2isize,
  /// A octal base — `0o`.
  Oct = 8isize,
  /// A decimal base — `123456`.
  Dec = 10isize,
  /// A hexadecimal base — `0x`.
  Hex = 16isize,
}

impl std::fmt::Display for Base {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Bin => write!(f, "binary"),
      Self::Oct => write!(f, "octal"),
      Self::Dec => write!(f, "decimal"),
      Self::Hex => write!(f, "hexadecimal"),
    }
  }
}
