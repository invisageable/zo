/// Check if a single byte is a 7-bit characters code of `"`, `'` or `backtick`
/// symbols.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::quotecase;
///
/// assert!(quotecase::is_quote('\''));
/// assert!(quotecase::is_quote('"'));
/// ```
#[inline(always)]
pub const fn is_quote(ch: char) -> bool {
  is_quote_backtick(ch) | is_quote_single(ch) | is_quote_double(ch)
}

/// Check if a single byte is a 7-bit characters code of `backtick` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::quotecase;
///
/// assert!(quotecase::is_quote_double('"'));
/// assert!(!quotecase::is_quote_double('!'));
/// ```
#[inline(always)]
pub const fn is_quote_backtick(ch: char) -> bool {
  matches!(ch, '`')
}

#[inline(always)]
/// Check if a single byte is a 7-bit characters code of `'` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::quotecase;
///
/// assert!(quotecase::is_quote_single('\''));
/// assert!(!quotecase::is_quote_single('^'));
/// ```
pub const fn is_quote_single(ch: char) -> bool {
  matches!(ch, '\'')
}

/// Check if a single byte is a 7-bit characters code of `"` symbol.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::quotecase;
///
/// assert!(quotecase::is_quote_double('"'));
/// assert!(!quotecase::is_quote_double('?'));
/// ```
#[inline(always)]
pub const fn is_quote_double(ch: char) -> bool {
  matches!(ch, '"')
}

/// Gets the `uppercase` name from a single byte.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::quotecase;
///
/// assert_eq!(quotecase::of_name('\''), Some("quote single"));
/// assert_eq!(quotecase::of_name('"'), Some("quote double"));
/// assert_eq!(quotecase::of_name('`'), Some("quote backtick"));
/// assert_eq!(quotecase::of_name(','), None);
/// ```
#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    b if is_quote_backtick(b) => "quote backtick",
    b if is_quote_single(b) => "quote single",
    b if is_quote_double(b) => "quote double",
    _ => return None,
  };

  Some(name)
}
