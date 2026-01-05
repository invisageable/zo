use super::identcase::is_underscore;

/// Checks if a single byte is a 7-bit characters code of digit character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::numbercase;
///
/// assert!(numbercase::is_number(b'2'));
/// assert!(!numbercase::is_number(b'a'));
/// ```
#[inline(always)]
pub const fn is_number(b: u8) -> bool {
  b.is_ascii_digit()
}

/// Checks if a single byte is a 7-bit characters code of digit `0` character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::numbercase;
///
/// assert!(numbercase::is_number_zero(b'0'));
/// assert!(!numbercase::is_number_zero(b'2'));
/// ```
#[inline(always)]
pub const fn is_number_zero(b: u8) -> bool {
  matches!(b, b'0')
}

/// Checks if a single byte is a 7-bit characters code of digit [1,9] character.
///
/// ## examples.
///
/// ```
/// use swisskit::case::bitcase::numbercase;
///
/// assert!(numbercase::is_number_non_zero(b'2'));
/// assert!(!numbercase::is_number_non_zero(b'a'));
/// ```
#[inline(always)]
pub const fn is_number_non_zero(b: u8) -> bool {
  matches!(b, b'1'..=b'9') || is_underscore(b)
}

/// Checks if a single byte is a 7-bit characters code of hex digit character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::numbercase;
///
/// assert!(numbercase::is_number_hex(b'f'));
/// assert!(!numbercase::is_number_hex(b'R'));
/// ```
#[inline(always)]
pub const fn is_number_hex(b: u8) -> bool {
  b.is_ascii_hexdigit()
}

/// Checks if a single byte is a 7-bit characters code of octal digit character.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::numbercase;
///
/// assert!(numbercase::is_number_oct(b'6'));
/// assert!(!numbercase::is_number_oct(b'9'));
/// ```
#[inline(always)]
pub const fn is_number_oct(b: u8) -> bool {
  matches!(b, b'0'..=b'7')
}

/// Checks if a single byte is a 7-bit characters code of binary digit.
///
/// #### examples.
///
/// ```
/// use swisskit::case::bitcase::numbercase;
///
/// assert!(numbercase::is_number_bin(b'0'));
/// assert!(!numbercase::is_number_bin(b'2'));
/// ```
#[inline(always)]
pub const fn is_number_bin(b: u8) -> bool {
  matches!(b, b'0'..=b'1')
}

/// Gets the `number` name from a single byte.
#[inline]
pub const fn of_name(byte: u8) -> Option<&'static str> {
  let name = match byte {
    b if is_number_hex(b) => "number hex",
    b if is_number_oct(b) => "number octal",
    b if is_number_bin(b) => "number binary",
    _ => return None,
  };

  Some(name)
}
