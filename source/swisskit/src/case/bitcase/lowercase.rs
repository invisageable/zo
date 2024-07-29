/// Checks if a byte is a 7-bit characters code of `lowercase` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::lowercase;
///
/// assert!(lowercase::is_lowercase(b'b'));
/// assert!(!lowercase::is_lowercase(b'A'));
/// ```
#[inline]
pub fn is_lowercase<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_lowercase()
}
