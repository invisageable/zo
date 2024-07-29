#[inline]
pub fn is_punctuation<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b',' | b':' | b';')
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b',' => "comma",
    b':' => "colon",
    b';' => "semicolon",
    _ => return None,
  };

  Some(name)
}
