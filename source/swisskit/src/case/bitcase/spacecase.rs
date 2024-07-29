/// Checks if a byte is a 7-bit characters code of whitespace symbols.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::spacecase;
///
/// assert!(spacecase::is_space(b' '));
/// assert!(!spacecase::is_space(b'_'));
/// ```
#[inline]
pub fn is_space<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_whitespace()
}

/// Gets the `whitespace` name from a byte.
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
