/// Checks if a byte is a 7-bit characters code of `uppercase` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::uppercase;
///
/// assert!(uppercase::is_uppercase(b'A'));
/// assert!(!uppercase::is_uppercase(b'b'));
/// ```
#[inline]
pub fn is_uppercase<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_uppercase()
}
