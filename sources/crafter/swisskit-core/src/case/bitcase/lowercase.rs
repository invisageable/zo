/// Checks if a single byte is a 7-bit characters code of `lowercase` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::lowercase;
///
/// assert!(lowercase::is_lowercase(b'b'));
/// assert!(!lowercase::is_lowercase(b'A'));
/// ```
#[inline(always)]
pub const fn is_lowercase(b: u8) -> bool {
  b.is_ascii_lowercase()
}

/// Converts an alphabetic character to a lower alphabetic character.
#[inline]
pub const fn to_lowercase_ascii(ch: u8) -> Option<u8> {
  match ch {
    b'a'..=b'z' => Some(ch),
    b'A'..=b'Z' => Some(ch - b'A' + b'a'),
    _ => None,
  }
}
