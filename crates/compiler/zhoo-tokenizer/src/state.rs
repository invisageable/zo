#[derive(Debug, PartialEq)]
pub(crate) enum TokenizerState {
  Start,
  Space,
  Zero,
  Hex,
  Comment,
  Int,
  Float,
  Ident,
  Op,
  Punctuation,
  Group,
  Unknown,
  End,
}
