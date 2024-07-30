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
#[inline]
pub fn is_ident<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_alphabetic()
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
#[inline]
pub fn is_ident_start<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_ident(byte) || is_underscore(byte)
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
#[inline]
pub fn is_ident_continue<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  is_ident(byte) || is_number(byte) || is_underscore(byte)
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
#[inline]
pub fn is_underscore<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'_'
}

/// Gets the `ident` name from a single byte.
pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_ident_start(b) => "ident start",
    b if is_ident_continue(b) => "ident continue",
    b if is_underscore(b) => "underscore",
    _ => return None,
  };

  Some(name)
}
