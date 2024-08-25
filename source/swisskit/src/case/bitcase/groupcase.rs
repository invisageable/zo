/// Checks if a single byte is a 7-bit characters code of `[`, `]`, `(`, `)`,
/// `{` or `}` character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::groupcase;
///
/// assert!(groupcase::is_group(b'{'));
/// assert!(!groupcase::is_group(b'"'));
/// ```
#[inline(always)]
pub const fn is_group(b: u8) -> bool {
  matches!(b, b'[' | b']' | b'(' | b')' | b'{' | b'}')
}

/// Gets the `group` name from a single byte.
#[inline]
pub const fn of_name(byte: u8) -> Option<&'static str> {
  let name = match byte {
    b'[' => "bracket open",
    b']' => "bracket close",
    b'(' => "paren open",
    b')' => "paren close",
    b'{' => "brace open",
    b'}' => "brace close",
    _ => return None,
  };

  Some(name)
}
