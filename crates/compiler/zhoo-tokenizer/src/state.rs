#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub(crate) enum TokenizerState {
  Start,
  Space,
  Comment,
  Zero,
  Hex,
  Oct,
  Bin,
  Int,
  Float,
  Ident,
  Notation,
  Op,
  Punctuation,
  Group,
  Quote,
  Char,
  Str,
  Unknown,
}
