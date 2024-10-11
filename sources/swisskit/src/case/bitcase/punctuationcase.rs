/// Checks if a single byte is a 7-bit characters code of punctuation character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::punctuationcase;
///
/// assert!(punctuationcase::is_punctuation(b','));
/// assert!(!punctuationcase::is_punctuation(b'0'));
/// ```
#[inline(always)]
pub const fn is_punctuation(b: u8) -> bool {
  matches!(
    b,
    b'.'
      | b'='
      | b'+'
      | b'-'
      | b'*'
      | b'/'
      | b'%'
      | b'^'
      | b'&'
      | b'|'
      | b'?'
      | b'!'
      | b'<'
      | b'>'
      | b'#'
      | b','
      | b':'
      | b';'
  )
}

/// Checks if a single byte is a 7-bit characters code of period character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::punctuationcase;
///
/// assert!(punctuationcase::is_dot(b'.'));
/// assert!(!punctuationcase::is_dot(b'0'));
/// ```
#[inline(always)]
pub const fn is_dot(b: u8) -> bool {
  matches!(b, b'.')
}

/// Gets the `punctuation` name from a single byte.
#[inline]
pub const fn of_name(byte: u8) -> Option<&'static str> {
  let name = match byte {
    b'.' => "period",
    b'=' => "equal",
    b'+' => "plus",
    b'*' => "times",
    b'/' => "slash",
    b'%' => "percent",
    b'^' => "circumflex",
    b'&' => "ampersant",
    b'|' => "pipe",
    b'?' => "question",
    b'!' => "exclamation",
    b'<' => "less than",
    b'>' => "greater than",
    b',' => "comma",
    b':' => "colon",
    b';' => "semicolon",
    _ => return None,
  };

  Some(name)
}
