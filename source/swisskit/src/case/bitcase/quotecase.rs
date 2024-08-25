/// Check if a single byte is a 7-bit characters code of `"`, `'` or `backtick`
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
#[inline(always)]
pub const fn is_quote(b: u8) -> bool {
  is_quote_single(b) | is_quote_double(b)
}

/// Check if a single byte is a 7-bit characters code of `'` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert!(quotecase::is_quote_single(b'\''));
/// assert!(!quotecase::is_quote_single(b'^'));
/// ```
#[inline(always)]
pub const fn is_quote_single(b: u8) -> bool {
  matches!(b, b'\'')
}

/// Check if a single byte is a 7-bit characters code of `"` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert!(quotecase::is_quote_double(b'"'));
/// assert!(!quotecase::is_quote_double(b'?'));
/// ```
#[inline(always)]
pub const fn is_quote_double(b: u8) -> bool {
  matches!(b, b'"')
}

/// Check if a single byte is a 7-bit characters code of `backtick` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::quotecase;
///
/// assert!(quotecase::is_quote_double(b'"'));
/// assert!(!quotecase::is_quote_double(b'!'));
/// ```
#[inline(always)]
pub const fn is_quote_backtick(b: u8) -> bool {
  matches!(b, b'`')
}

/// Gets the `uppercase` name from a single byte.
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
#[inline]
pub const fn of_name(byte: u8) -> Option<&'static str> {
  let name = match byte {
    b if is_quote_single(b) => "quote single",
    b if is_quote_double(b) => "quote double",
    b if is_quote_backtick(b) => "quote backtick",
    _ => return None,
  };

  Some(name)
}
