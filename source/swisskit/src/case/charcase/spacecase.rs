/// Checks if a single byte is a 7-bit characters code of whitespace symbols.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::spacecase;
///
/// assert!(spacecase::is_space(b' '));
/// assert!(!spacecase::is_space(b'_'));
/// ```
#[inline(always)]
pub const fn is_space(ch: char) -> bool {
  ch.is_ascii_whitespace()
}

/// Gets the `whitespace` name from a single byte.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::spacecase;
///
/// assert_eq!(spacecase::of_name(b' '),  Some("space"));
/// assert_eq!(spacecase::of_name(b'\t'), Some("tab"));
/// assert_eq!(spacecase::of_name(b'\r'), Some("carriage return"));
/// assert_eq!(spacecase::of_name(b'*'), None);
/// ```
#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    ' ' => "space",
    '\t' => "tab",
    '\r' => "carriage return",
    _ => return None,
  };

  Some(name)
}
