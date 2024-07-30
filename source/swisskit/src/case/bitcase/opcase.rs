/// Checks if a single byte is a 7-bit characters code of operators character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::opcase;
///
/// assert!(opcase::is_op(b'+'));
/// assert!(!opcase::is_op(b'"'));
/// ```
#[inline]
pub fn is_op<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(
    byte.into(),
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
  )
}

/// Checks if a single byte is a 7-bit characters code of `.` character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::opcase;
///
/// assert!(opcase::is_period(b'.'));
/// assert!(!opcase::is_period(b','));
/// ```
#[inline]
pub fn is_period<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'.'
}

/// Gets the `operator` name from a single byte.
pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
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
    _ => return None,
  };

  Some(name)
}
