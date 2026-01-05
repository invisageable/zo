use super::identcase::is_underscore;

/// Checks if a single byte is a 7-bit characters code of digit character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::numbercase;
///
/// assert!(numbercase::is_number('2'));
/// assert!(!numbercase::is_number('a'));
/// ```
#[inline(always)]
pub const fn is_number(ch: char) -> bool {
  ch.is_ascii_digit()
}

/// Checks if a single byte is a 7-bit characters code of digit `0` character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::numbercase;
///
/// assert!(numbercase::is_number_zero('0'));
/// assert!(!numbercase::is_number_zero('2'));
/// ```
#[inline(always)]
pub const fn is_number_zero(ch: char) -> bool {
  matches!(ch, '0')
}

/// Checks if a single byte is a 7-bit characters code of digit [1,9] character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::charcase::numbercase;
///
/// assert!(numbercase::is_number_non_zero('2'));
/// assert!(!numbercase::is_number_non_zero('a'));
/// ```
#[inline(always)]
pub const fn is_number_non_zero(ch: char) -> bool {
  matches!(ch, '1'..='9') || is_underscore(ch)
}

/// Checks if a single byte is a 7-bit characters code of binary digit.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::numbercase;
///
/// assert!(numbercase::is_number_bin('0'));
/// assert!(!numbercase::is_number_bin('2'));
/// ```
#[inline(always)]
pub const fn is_number_bin(ch: char) -> bool {
  matches!(ch, '0'..='1')
}

/// Checks if a single byte is a 7-bit characters code of octal digit character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::numbercase;
///
/// assert!(numbercase::is_number_oct('6'));
/// assert!(!numbercase::is_number_oct('9'));
/// ```
#[inline(always)]
pub const fn is_number_oct(ch: char) -> bool {
  matches!(ch, '0'..='7')
}

/// Checks if a single byte is a 7-bit characters code of hex digit character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::charcase::numbercase;
///
/// assert!(numbercase::is_number_hex('f'));
/// assert!(!numbercase::is_number_hex('R'));
/// ```
#[inline(always)]
pub const fn is_number_hex(ch: char) -> bool {
  ch.is_ascii_hexdigit()
}

/// Gets the `number` name from a single byte.
#[inline]
pub fn of_name(ch: char) -> Option<&'static str> {
  let name = match ch {
    b if is_number(b) => "number",
    b if is_number_zero(b) => "number zero",
    b if is_number_non_zero(b) => "number non zero",
    b if is_number_bin(b) => "number binary",
    b if is_number_oct(b) => "number octal",
    b if is_number_hex(b) => "number hex",
    _ => return None,
  };

  Some(name)
}
