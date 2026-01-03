use super::numbercase::is_number;

/// Checks if a single byte is a 7-bit characters code of alphabetic character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::identcase;
///
/// assert!(identcase::is_ident(b'a'));
/// assert!(!identcase::is_ident(b'/'));
/// ```
#[inline(always)]
pub const fn is_ident(b: u8) -> bool {
  b.is_ascii_alphabetic()
}

/// Checks if a single byte is a 7-bit characters code of alphabetic or `_`
/// character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::identcase;
///
/// assert!(identcase::is_ident_start(b'a'));
/// assert!(!identcase::is_ident_start(b'2'));
/// ```
#[inline(always)]
pub const fn is_ident_start(b: u8) -> bool {
  is_ident(b) || is_underscore(b)
}

/// Checks if a single byte is a 7-bit characters code of alphabetic, digit or
/// `_` character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::identcase;
///
/// assert!(identcase::is_ident_continue(b'a'));
/// assert!(identcase::is_ident_continue(b'2'));
/// assert!(identcase::is_ident_continue(b'_'));
/// assert!(!identcase::is_ident_continue(b'/'));
/// ```
#[inline(always)]
pub const fn is_ident_continue(b: u8) -> bool {
  is_ident(b) || is_number(b) || is_underscore(b)
}

/// Checks if a single byte is a 7-bit characters code of `_` character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::identcase;
///
/// assert!(identcase::is_underscore(b'_'));
/// assert!(!identcase::is_underscore(b'2'));
/// ```
#[inline(always)]
pub const fn is_underscore(b: u8) -> bool {
  matches!(b, b'_')
}

/// Gets the `ident` name from a single byte.
#[inline]
pub const fn of_name(byte: u8) -> Option<&'static str> {
  let name = match byte {
    b if is_ident_start(b) => "ident start",
    b if is_ident_continue(b) => "ident continue",
    b if is_underscore(b) => "underscore",
    _ => return None,
  };

  Some(name)
}
