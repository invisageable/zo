#[inline]
pub fn is_lowercase<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_lowercase()
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_lowercase(b) => "lowercase",
    _ => return None,
  };

  Some(name)
}
