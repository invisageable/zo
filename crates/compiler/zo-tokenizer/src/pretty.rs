//! ...

use super::token::{Token, TokenKind};

impl std::fmt::Display for Token {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl std::fmt::Display for TokenKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      Self::Int(symbol, _) => write!(f, "{symbol}"),
      Self::Float(symbol) => write!(f, "{symbol}"),
      Self::Ident(symbol) => write!(f, "{symbol}"),
      Self::Char(symbol) => write!(f, "{symbol}"),
      Self::Str(symbol) => write!(f, "{symbol}"),
      Self::Kw(kw) => write!(f, "{kw}"),
      Self::Op(op) => write!(f, "{op}"),
      Self::Group(group) => write!(f, "{group}"),
      Self::Punctuation(punctuation) => write!(f, "{punctuation}"),
      Self::Unknwon => write!(f, "UNKNOWN"),
      Self::Eof => write!(f, "EOF"),
    }
  }
}
