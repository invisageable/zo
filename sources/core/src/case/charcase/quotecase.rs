#[inline]
pub fn is_quote<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_quote_single(byte) | is_quote_double(byte)
}

#[inline]
pub fn is_quote_single<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'\''
}

#[inline]
pub fn is_quote_double<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'"'
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_quote_single(b) => "quote single",
    b if is_quote_double(b) => "quote double",
    _ => return None,
  };

  Some(name)
}
