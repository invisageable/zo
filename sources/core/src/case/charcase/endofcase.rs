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
/// ```
/// use zo_core::case::charcase::endofcase;
///
/// assert!(endofcase::is_eof(b'\0'));
/// assert!(!endofcase::is_eof(b'_'));
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
/// ```
/// use zo_core::case::charcase::endofcase;
///
/// assert!(endofcase::is_eol(b'\n'));
/// assert!(!endofcase::is_eol(b'_'));
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
