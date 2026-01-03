/// Checks if a single byte is a 7-bit characters code of `\0` or `\n`
/// character.
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
#[inline(always)]
pub const fn is_eo(b: u8) -> bool {
  is_eof(b) || is_eol(b)
}

/// Checks if a single byte is a 7-bit characters code of `\0` character.
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
#[inline(always)]
pub const fn is_eof(b: u8) -> bool {
  matches!(b, b'\0')
}

/// Checks if a single byte is a 7-bit characters code of `\n` character.
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
#[inline(always)]
pub const fn is_eol(b: u8) -> bool {
  matches!(b, b'\n')
}

/// Gets the `endof` name from a single byte.
#[inline]
pub fn of_name(b: u8) -> Option<&'static str> {
  let name = match b {
    b'\0' => "eof",
    b'\n' => "eol",
    _ => return None,
  };

  Some(name)
}
