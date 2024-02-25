#[inline]
pub fn is_end_of<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_eof(byte) || is_eol(byte)
}

/// `null` character.
///
/// byte: `00000000`
///
/// ## example.
///
/// ```rs
/// use swiss_kit_case::charcase::endofcase;
/// assert!(endofcase::is_eof('\0'));
/// assert!(endofcase::is_eof(''));
/// ```
//
#[inline]
pub fn is_eof<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'\0'
}

/// end of line (newline).
///
/// byte: `00001010`
///
/// ## example.
///
/// ```rs
/// use swiss_kit_case::charcase::endofcase;
/// assert!(endofcase::is_eol('\0'));
/// assert!(endofcase::is_eol(''));
/// ```
//
#[inline]
pub fn is_eol<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'\n'
}

pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b'\0' => "eof",
    b'\n' => "eol",
    _ => return None,
  };

  Some(name)
}
