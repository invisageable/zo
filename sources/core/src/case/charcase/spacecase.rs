#[inline]
pub fn is_space<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_whitespace()
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b' ' => "space",
    b'\t' => "tab",
    b'\r' => "carriage return",
    _ => return None,
  };

  Some(name)
}
