/// Checks if a single byte is a 7-bit characters code of `[`, `]`, `(`, `)`,
/// `{` or `}` character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::groupcase;
///
/// assert!(groupcase::is_group('{'));
/// assert!(!groupcase::is_group('"'));
/// ```
#[inline(always)]
pub const fn is_group(ch: char) -> bool {
  matches!(ch, '[' | ']' | '(' | ')' | '{' | '}')
}

#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    '[' => "bracket open",
    ']' => "bracket close",
    '(' => "paren open",
    ')' => "paren close",
    '{' => "brace open",
    '}' => "brace close",
    _ => return None,
  };

  Some(name)
}
