/// Checks if a single byte is a 7-bit characters code of punctuation character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::punctuationcase;
///
/// assert!(punctuationcase::is_punctuation(b','));
/// assert!(!punctuationcase::is_punctuation(b'!'));
/// ```
#[inline]
pub fn is_punctuation<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b',' | b':' | b';')
}

/// Gets the `punctuation` name from a single byte.
pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b',' => "comma",
    b':' => "colon",
    b';' => "semicolon",
    _ => return None,
  };

  Some(name)
}
