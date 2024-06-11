//! ...

use super::numbercase::is_number;

#[inline]
pub fn is_ident<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_alphabetic()
}

#[inline]
pub fn is_ident_start<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_ident(byte) || is_underscore(byte)
}

#[inline]
pub fn is_ident_continue<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_ident(byte) || is_number(byte) || is_underscore(byte)
}

#[inline]
pub fn is_underscore<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'_'
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_ident_start(b) => "ident start",
    b if is_ident_continue(b) => "ident continue",
    b if is_underscore(b) => "underscore",
    _ => return None,
  };

  Some(name)
}
