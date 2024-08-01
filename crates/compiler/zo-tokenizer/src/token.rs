//! `zo` source code are splitted into the following kinds of tokens:
//!
//! * end of file.
//! * unknown characters.
//! * spaces.
//! * end of line.
//! * integers.
//! * floats.
//! * punctuations.
//! * groups.
//! * identifiers.
//! * keywords.
//! * characters.
//! * strings.

pub mod group;
pub mod int;
pub mod kw;
pub mod punctuation;

use zo_interner::interner::symbol::Symbol;

use swisskit::span::Span;

/// The representation of a token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Token {
  /// See [`TokenKind`].
  pub kind: TokenKind,
  /// See [`Span`].
  pub span: Span,
}

impl Token {
  /// A constant, it is used as a placeholder.
  pub const EOF: Self = Self::new(TokenKind::Eof, Span::ZERO);

  /// Creates a new token instance with span.
  #[inline]
  pub const fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }

  /// Check if the token kind match.
  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    self.kind.is(kind)
  }
}

/// The representation of a token kind.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TokenKind {
  /// end of file — `'\0'`.
  Eof,
  /// unknown character.
  Unknown,
  /// a space — `' '`.
  Space,
  /// end of line - `'\n'`.
  Eol,
  /// integer.
  Int(Symbol),
  /// float.
  Float(Symbol),
  /// punctuation.
  Punctuation(punctuation::Punctuation),
  /// group.
  Group(group::Group),
  /// identifier.
  Ident(Symbol),
  /// keyword.
  Kw(kw::Kw),
  /// character.
  Char(Symbol),
  /// string.
  Str(Symbol),
}

impl TokenKind {
  /// Checks the equality of a token kind.
  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    *self == kind
  }
}
