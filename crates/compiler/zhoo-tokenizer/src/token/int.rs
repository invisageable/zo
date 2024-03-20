//! ...

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BaseInt {
  B16 = 16, // hexadecimal
  B10 = 10, // decimal
  B8 = 8,   // octal
  B2 = 2,   // binary
}
