//! ...

use super::identcase::is_underscore;

#[inline]
pub fn is_number<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_digit()
}

#[inline]
pub fn is_number_zero<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'0'
}

#[inline]
pub fn is_number_continue<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'1'..=b'9') || is_underscore(byte)
}

#[inline]
pub fn is_number_hex<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_hexdigit()
}

#[inline]
pub fn is_number_oct<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'0'..=b'7')
}

#[inline]
pub fn is_number_bin<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'0'..=b'1')
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_number(b) => "number",
    b if is_number_hex(b) => "number hex",
    b if is_number_oct(b) => "number octal",
    b if is_number_bin(b) => "number binary",
    _ => return None,
  };

  Some(name)
}
