#[inline]
pub fn is_ident(c: char) -> bool {
  c.is_alphabetic() || is_underscore(c)
}

#[inline]
pub fn is_underscore(c: char) -> bool {
  c == '\u{005F}'
}
