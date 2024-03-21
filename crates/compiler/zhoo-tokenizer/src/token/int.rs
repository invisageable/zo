//! ...

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BaseInt {
  Hex = 16, // hexadecimal
  Dec = 10, // decimal
  Oct = 8,  // octal
  Bin = 2,  // binary
}
