/// Checks if a character is a 7-bit characters code of end of file character.
#[inline]
pub fn is_eof(c: char) -> bool {
  c == '\u{0}'
}

/// Checks if a character is a 7-bit characters code of end of line character.
#[inline]
pub fn is_eol(c: char) -> bool {
  c == '\u{000A}'
}
