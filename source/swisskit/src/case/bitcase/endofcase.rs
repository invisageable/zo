/// Checks if a byte is a 7-bit characters code of `\0` or `\n` character.
///
/// It will detect new line and end of file.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::endofcase;
///
/// assert!(endofcase::is_eo(b'\0'));
/// assert!(endofcase::is_eo(b'\n'));
/// assert!(!endofcase::is_eo(b'o'));
/// ```
#[inline]
pub fn is_eo<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_eof(byte) || is_eol(byte)
}

/// Checks if a byte is a 7-bit characters code of `\0` character.
///
/// Also named: `Null`.
///
/// bin: `00000000`
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::endofcase;
///
/// assert!(endofcase::is_eof(b'\0'));
/// assert!(!endofcase::is_eof(b'_'));
/// ```
#[inline]
pub fn is_eof<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'\0'
}

/// Checks if a byte is a 7-bit characters code of `\n` character.
///
/// Also named: `Line Feed`.
///
/// bin: `00001010`
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::endofcase;
///
/// assert!(endofcase::is_eol(b'\n'));
/// assert!(!endofcase::is_eol(b'_'));
/// ```
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
