//! Token types for the fret.oz configuration format.

use std::fmt;

/// Token type for the fret.oz format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
  String,
  Number,
  Identifier,
  Pack,
  At,           // @
  LeftParen,    // (
  RightParen,   // )
  LeftBracket,  // [
  RightBracket, // ]
  Comma,        // ,
  Colon,        // :
  Equal,        // =
  Eof,
  Error,
}

/// Byte-offset span into source text — no owned strings.
#[derive(Debug, Clone, Copy)]
pub struct Token {
  /// The type of this token.
  pub kind: TokenKind,
  /// Byte offset of the first character.
  pub start: usize,
  /// Byte offset past the last character.
  pub end: usize,
}

impl Token {
  /// Create a new token with the given kind and byte range.
  #[inline]
  pub fn new(kind: TokenKind, start: usize, end: usize) -> Self {
    Self { kind, start, end }
  }

  /// Returns the text slice from `source` that this token
  /// spans.
  #[inline]
  pub fn lexeme<'a>(&self, source: &'a str) -> &'a str {
    &source[self.start..self.end]
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    matches!(self.kind, TokenKind::Eof)
  }

  /// Get the length of this token in bytes.
  #[inline(always)]
  pub fn len(&self) -> usize {
    self.end - self.start
  }
}

impl fmt::Display for TokenKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TokenKind::String => write!(f, "string"),
      TokenKind::Number => write!(f, "number"),
      TokenKind::Identifier => write!(f, "identifier"),
      TokenKind::Pack => write!(f, "pack"),
      TokenKind::At => write!(f, "@"),
      TokenKind::LeftParen => write!(f, "("),
      TokenKind::RightParen => write!(f, ")"),
      TokenKind::LeftBracket => write!(f, "["),
      TokenKind::RightBracket => write!(f, "]"),
      TokenKind::Comma => write!(f, ","),
      TokenKind::Colon => write!(f, ":"),
      TokenKind::Equal => write!(f, "="),
      TokenKind::Eof => write!(f, "end of file"),
      TokenKind::Error => write!(f, "error"),
    }
  }
}
