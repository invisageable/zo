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
#[inline]
pub fn is_group<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'[' | b']' | b'(' | b')' | b'{' | b'}')
}

/// Gets the `group` name from a single byte.
pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
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
