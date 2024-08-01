/// The representation of integer base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BaseInt {
  /// hexadecimal.
  Hex = 16,
  /// decimal.
  Dec = 10,
  /// octal.
  Oct = 8,
  /// binary.
  Bin = 2,
}
