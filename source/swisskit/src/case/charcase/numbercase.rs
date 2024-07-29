#[inline]
pub fn is_number(c: char) -> bool {
  c.is_digit(10)
}

#[inline]
pub fn is_number_zero(c: char) -> bool {
  c == '0'
}

#[inline]
pub fn is_number_continue(c: char) -> bool {
  matches!(c, '1'..='9')
}

#[inline]
pub fn is_number_hex(c: char) -> bool {
  matches!(c, '0'..='9' | 'a'..='f' | 'A'..='F')
}
