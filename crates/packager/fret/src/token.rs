//! Token types for the fret.oz configuration format.
//!
//! This module defines the token types produced by the lexer.
//! Following the "Speed is Law" principle, tokens are designed to be
//! lightweight and avoid heap allocations where possible.

use std::fmt;

/// Token type for the fret.oz format.
/// Designed for zero-allocation tokenization where possible.
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

/// A token with its source location.
/// Tokens store byte offsets into the source text rather than
/// owning strings, enabling zero-allocation tokenization.
#[derive(Debug, Clone, Copy)]
pub struct Token {
  pub kind: TokenKind,
  pub start: usize,
  pub end: usize,
}

impl Token {
  /// Create a new token with the given kind and byte range.
  #[inline]
  pub fn new(kind: TokenKind, start: usize, end: usize) -> Self {
    Self { kind, start, end }
  }

  /// Get the lexeme (text) of this token from the source.
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
