/// Checks if a single byte is a 7-bit characters code of `lowercase` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::lowercase;
///
/// assert!(lowercase::is_lowercase('b'));
/// assert!(!lowercase::is_lowercase('A'));
/// ```
#[inline(always)]
pub const fn is_lowercase(ch: char) -> bool {
  ch.is_ascii_lowercase()
}

/// Converts an alphabetic character to a lower alphabetic character.
#[inline]
pub const fn to_lowercase_ascii(ch: char) -> Option<char> {
  match ch {
    'a'..='z' => Some(ch),
    'A'..='Z' => Some((ch as u8 - b'A' + b'a') as char),
    _ => None,
  }
}

#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    b if is_lowercase(b) => "lowercase",
    _ => return None,
  };

  Some(name)
}
