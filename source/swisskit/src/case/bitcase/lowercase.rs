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
