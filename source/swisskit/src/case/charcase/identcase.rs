use super::numbercase::is_number;

/// Checks if a single byte is a 7-bit characters code of alphabetic character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::identcase;
///
/// assert!(identcase::is_ident('a'));
/// assert!(!identcase::is_ident('/'));
/// ```
#[inline(always)]
pub const fn is_ident(ch: char) -> bool {
  ch.is_ascii_alphabetic()
}

/// Checks if a single byte is a 7-bit characters code of alphabetic or `_`
/// character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::identcase;
///
/// assert!(identcase::is_ident_start('a'));
/// assert!(!identcase::is_ident_start('2'));
/// ```
#[inline(always)]
pub const fn is_ident_start(ch: char) -> bool {
  is_ident(ch) || is_underscore(ch)
}

/// Checks if a single byte is a 7-bit characters code of alphabetic, digit or
/// `_` character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::identcase;
///
/// assert!(identcase::is_ident_continue('a'));
/// assert!(identcase::is_ident_continue('2'));
/// assert!(identcase::is_ident_continue('_'));
/// assert!(!identcase::is_ident_continue('/'));
/// ```
#[inline(always)]
pub const fn is_ident_continue(ch: char) -> bool {
  is_ident(ch) || is_number(ch) || is_underscore(ch)
}

/// Checks if a single byte is a 7-bit characters code of `_` character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::identcase;
///
/// assert!(identcase::is_underscore('_'));
/// assert!(!identcase::is_underscore('2'));
/// ```
#[inline(always)]
pub const fn is_underscore(ch: char) -> bool {
  matches!(ch, '_')
}

/// Gets the `ident` name from a single byte.
#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    b if is_ident_start(b) => "ident start",
    b if is_ident_continue(b) => "ident continue",
    b if is_underscore(b) => "underscore",
    _ => return None,
  };

  Some(name)
}
