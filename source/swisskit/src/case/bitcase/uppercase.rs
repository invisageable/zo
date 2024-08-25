/// Checks if a single byte is a 7-bit characters code of `uppercase` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::uppercase;
///
/// assert!(uppercase::is_uppercase(b'A'));
/// assert!(!uppercase::is_uppercase(b'b'));
/// ```
#[inline(always)]
pub const fn is_uppercase(b: u8) -> bool {
  b.is_ascii_uppercase()
}
