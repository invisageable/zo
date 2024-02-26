#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(crate) enum TokenizerState {
  Start,
  Space,
  Comment,
  Zero,
  Hex,
  Int,
  Float,
  Ident,
  Op,
  Punctuation,
  Group,
  Unknown,
  End,
}
