/// The representation of integer base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Base {
  /// binary as `0b`.
  Bin = 2,
  /// octal as `0o`.
  Oct = 8,
  /// decimal as `123456`.
  Dec = 10,
  /// hexadecimal as `0x`.
  Hex = 16,
}

impl std::fmt::Display for Base {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Base::Bin => write!(f, "binary"),
      Self::Oct => write!(f, "octal"),
      Self::Dec => write!(f, "decimal"),
      Self::Hex => write!(f, "hexadecimal"),
    }
  }
}
