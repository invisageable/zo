/// Checks if a single byte is a 7-bit characters code of `.`, `,` character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::delimcase;
///
/// assert!(delimcase::is_delim('{'));
/// assert!(!delimcase::is_delim('"'));
/// ```
#[inline(always)]
pub const fn is_delim(ch: char) -> bool {
  matches!(ch, '.' | ',')
}

#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    '.' => "dot",
    ',' => "comma",
    _ => return None,
  };

  Some(name)
}
