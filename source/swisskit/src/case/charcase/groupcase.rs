#[inline]
pub fn is_group(c: char) -> bool {
  matches!(c, '[' | ']' | '(' | ')' | '{' | '}')
}
