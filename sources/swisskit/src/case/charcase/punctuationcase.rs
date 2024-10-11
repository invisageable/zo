/// Checks if a single byte is a 7-bit characters code of punctuation character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::punctuationcase;
///
/// assert!(punctuationcase::is_punctuation(','));
/// assert!(!punctuationcase::is_punctuation('0'));
/// ```
#[inline(always)]
pub const fn is_punctuation(ch: char) -> bool {
  matches!(
    ch,
    '='
      | '+'
      | '-'
      | '*'
      | '/'
      | '%'
      | '^'
      | '&'
      | '|'
      | '?'
      | '!'
      | '<'
      | '>'
      | '#'
      | ','
      | '.'
      | ':'
      | ';'
  )
}

/// Checks if a single byte is a 7-bit characters code of dot character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::punctuationcase;
///
/// assert!(punctuationcase::is_dot('.'));
/// assert!(!punctuationcase::is_dot('0'));
/// ```
#[inline(always)]
pub const fn is_dot(ch: char) -> bool {
  matches!(ch, '.')
}

#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    '=' => "equal",
    '+' => "plus",
    '*' => "times",
    '/' => "slash",
    '%' => "percent",
    '^' => "circumflex",
    '&' => "ampersant",
    '|' => "pipe",
    '?' => "question",
    '!' => "exclamation",
    '<' => "less than",
    '>' => "greater than",
    ',' => "comma",
    '.' => "dot",
    ':' => "colon",
    ';' => "semicolon",
    _ => return None,
  };

  Some(name)
}
