//! ...

#[inline]
pub fn is_uppercase<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_uppercase()
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_uppercase(b) => "uppercase",
    _ => return None,
  };

  Some(name)
}
