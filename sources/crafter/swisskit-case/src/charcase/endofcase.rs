/// Checks if a single byte is a 7-bit characters code of `\0` or `\n`
/// character.
///
/// It will detect new line and end of file.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::endofcase;
///
/// assert!(endofcase::is_eo('\0'));
/// assert!(endofcase::is_eo('\n'));
/// assert!(!endofcase::is_eo('o'));
/// ```
#[inline(always)]
pub const fn is_eo(ch: char) -> bool {
  is_eof(ch) || is_eol(ch)
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
/// use swisskit::case::charcase::endofcase;
///
/// assert!(endofcase::is_eof('\0'));
/// assert!(!endofcase::is_eof('_'));
/// ```
#[inline(always)]
pub const fn is_eof(ch: char) -> bool {
  matches!(ch, '\0')
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
/// use swisskit::case::charcase::endofcase;
///
/// assert!(endofcase::is_eol('\n'));
/// assert!(!endofcase::is_eol('_'));
/// ```
#[inline(always)]
pub const fn is_eol(ch: char) -> bool {
  matches!(ch, '\n')
}

#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    '\0' => "eof",
    '\n' => "eol",
    _ => return None,
  };

  Some(name)
}
