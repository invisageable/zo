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
pub const fn is_uppercase(ch: char) -> bool {
  ch.is_ascii_uppercase()
}

#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    b if is_uppercase(b) => "uppercase",
    _ => return None,
  };

  Some(name)
}
