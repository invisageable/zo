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
#[inline]
pub fn is_number<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_digit()
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
#[inline]
pub fn is_number_zero<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into() == b'0'
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
#[inline]
pub fn is_number_non_zero<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'1'..=b'9') || is_underscore(byte)
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
#[inline]
pub fn is_number_hex<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  byte.into().is_ascii_hexdigit()
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
#[inline]
pub fn is_number_oct<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'0'..=b'7')
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
#[inline]
pub fn is_number_bin<B>(byte: B) -> bool
where
  B: Into<u8> + Copy,
{
  matches!(byte.into(), b'0'..=b'1')
}

/// Gets the `number` name from a single byte.
pub fn of_name<B>(byte: B) -> Option<&'static str>
where
  B: Into<u8> + Copy,
{
  let name = match byte.into() {
    b if is_number_hex(b) => "number hex",
    b if is_number_oct(b) => "number octal",
    b if is_number_bin(b) => "number binary",
    _ => return None,
  };

  Some(name)
}
