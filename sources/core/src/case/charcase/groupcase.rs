//! ...

#[inline]
pub fn is_group<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'[' | b']' | b'(' | b')' | b'{' | b'}')
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b'[' => "bracket open",
    b']' => "bracket close",
    b'(' => "paren open",
    b')' => "paren close",
    b'{' => "brace open",
    b'}' => "brace close",
    _ => return None,
  };

  Some(name)
}
