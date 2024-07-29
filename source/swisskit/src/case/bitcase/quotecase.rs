/// Check if a byte is a 7-bit characters code of `"`, `'` or `backtick`
/// symbols.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert!(quotecase::is_quote(b'\''));
/// assert!(quotecase::is_quote(b'"'));
/// ```
#[inline]
pub fn is_quote<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_quote_single(byte) | is_quote_double(byte)
}

/// Check if a byte is a 7-bit characters code of `'` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert!(quotecase::is_quote_single(b'\''));
/// assert!(!quotecase::is_quote_single(b'^'));
/// ```
#[inline]
pub fn is_quote_single<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'\''
}

/// Check if a byte is a 7-bit characters code of `"` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert!(quotecase::is_quote_double(b'"'));
/// assert!(!quotecase::is_quote_double(b'?'));
/// ```
#[inline]
pub fn is_quote_double<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'"'
}

/// Check if a byte is a 7-bit characters code of `backtick` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert!(quotecase::is_quote_double(b'"'));
/// assert!(!quotecase::is_quote_double(b'!'));
/// ```
#[inline]
pub fn is_quote_backtick<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'`'
}

/// Gets the `uppercase` name from a byte.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert_eq!(quotecase::of_name(b'\''), Some("quote single"));
/// assert_eq!(quotecase::of_name(b'"'), Some("quote double"));
/// assert_eq!(quotecase::of_name(b'`'), Some("quote backtick"));
/// assert_eq!(quotecase::of_name(b','), None);
/// ```
pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_quote_single(b) => "quote single",
    b if is_quote_double(b) => "quote double",
    b if is_quote_backtick(b) => "quote backtick",
    _ => return None,
  };

  Some(name)
}
