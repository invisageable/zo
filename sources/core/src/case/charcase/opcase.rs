//! ...

#[inline]
pub fn is_dot<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'.'
}

#[inline]
pub fn is_op<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(
    byte.into(),
    b'='
      | b'+'
      | b'-'
      | b'*'
      | b'/'
      | b'%'
      | b'^'
      | b'&'
      | b'|'
      | b'?'
      | b'!'
      | b'<'
      | b'>'
      | b'#'
  )
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b'=' => "equal",
    b'+' => "plus",
    b'*' => "times",
    b'/' => "slash",
    b'%' => "percent",
    b'^' => "circumflex",
    b'&' => "ampersant",
    b'|' => "pipe",
    b'?' => "question",
    b'!' => "exclamation",
    b'<' => "less than",
    b'>' => "greater than",
    _ => return None,
  };

  Some(name)
}
