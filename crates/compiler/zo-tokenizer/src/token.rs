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

use group::Group;
use kw::Kw;
use punctuation::Punctuation;

use zo_interner::interner::symbol::Symbol;

use swisskit::span::Span;

use smol_str::{SmolStr, ToSmolStr};

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

impl ToSmolStr for Token {
  fn to_smolstr(&self) -> smol_str::SmolStr {
    self.kind.to_smolstr()
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
  Int(Symbol, int::BaseInt),
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

  #[inline]
  pub fn is_assignement(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Equal)
        | Self::Punctuation(Punctuation::PlusEqual)
        | Self::Punctuation(Punctuation::MinusEqual)
        | Self::Punctuation(Punctuation::AsteriskEqual)
        | Self::Punctuation(Punctuation::SlashEqual)
        | Self::Punctuation(Punctuation::PercentEqual)
        | Self::Punctuation(Punctuation::CircumflexEqual)
        | Self::Punctuation(Punctuation::AmspersandEqual)
        | Self::Punctuation(Punctuation::PipeEqual)
        | Self::Punctuation(Punctuation::LessThanLessThanEqual)
        | Self::Punctuation(Punctuation::GreaterThanGreaterThanEqual)
    )
  }

  #[inline]
  pub fn is_conditional(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::AmpersandAmpersand)
        | Self::Punctuation(Punctuation::PipePipe)
    )
  }

  #[inline]
  pub fn is_comparison(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::EqualEqual)
        | Self::Punctuation(Punctuation::ExclamationEqual)
        | Self::Punctuation(Punctuation::LessThan)
        | Self::Punctuation(Punctuation::GreaterThan)
        | Self::Punctuation(Punctuation::LessThanEqual)
        | Self::Punctuation(Punctuation::GreaterThanEqual)
    )
  }

  #[inline]
  pub fn is_sum(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Plus)
        | Self::Punctuation(Punctuation::Minus)
    )
  }

  #[inline]
  pub fn is_exponent(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Asterisk)
        | Self::Punctuation(Punctuation::Slash)
        | Self::Punctuation(Punctuation::Percent)
    )
  }

  #[inline]
  pub fn is_calling(&self) -> bool {
    matches!(self, Self::Group(Group::ParenOpen))
  }

  #[inline]
  pub fn is_index(&self) -> bool {
    matches!(self, Self::Group(Group::BracketOpen))
  }

  #[inline]
  pub fn is_chaining(&self) -> bool {
    matches!(self, Self::Punctuation(Punctuation::Period))
  }

  #[inline]
  pub fn is_range(&self) -> bool {
    matches!(self, Self::Punctuation(Punctuation::PeriodPeriod))
  }

  #[inline]
  pub fn is_unop(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Exclamation)
        | Self::Punctuation(Punctuation::Minus)
    )
  }

  #[inline]
  pub fn is_binop(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Plus)
        | Self::Punctuation(Punctuation::Minus)
        | Self::Punctuation(Punctuation::Asterisk)
        | Self::Punctuation(Punctuation::Slash)
        | Self::Punctuation(Punctuation::Percent)
        | Self::Punctuation(Punctuation::Circumflex)
        | Self::Punctuation(Punctuation::EqualEqual)
        | Self::Punctuation(Punctuation::ExclamationEqual)
        | Self::Punctuation(Punctuation::LessThan)
        | Self::Punctuation(Punctuation::GreaterThan)
        | Self::Punctuation(Punctuation::LessThanEqual)
        | Self::Punctuation(Punctuation::GreaterThanEqual)
    )
  }

  #[inline]
  pub fn is_item(&self) -> bool {
    matches!(
      self,
      Self::Kw(Kw::Load)
        | Self::Kw(Kw::Val)
        | Self::Kw(Kw::Type)
        | Self::Kw(Kw::Ext)
        | Self::Kw(Kw::Struct)
        | Self::Kw(Kw::Fun)
    )
  }

  #[inline]
  pub fn is_var_local(&self) -> bool {
    matches!(self, Self::Kw(Kw::Imu) | Self::Kw(Kw::Mut))
  }
}

impl ToSmolStr for TokenKind {
  fn to_smolstr(&self) -> smol_str::SmolStr {
    match self {
      Self::Eof => SmolStr::new_inline("EOF"),
      Self::Unknown => SmolStr::new_inline("UNKNOWN"),
      Self::Space => SmolStr::new_inline("SPACE"),
      Self::Eol => SmolStr::new_inline("EOL"),
      Self::Int(sym, base) => {
        SmolStr::new_inline(format!("{sym}--base--{base:?}").as_str())
      }
      Self::Float(sym) => SmolStr::new_inline(format!("{sym}").as_str()),
      Self::Punctuation(punctuation) => {
        SmolStr::new_inline(format!("{punctuation}").as_str())
      }
      Self::Group(group) => SmolStr::new_inline(format!("{group}").as_str()),
      Self::Ident(sym) => SmolStr::new_inline(format!("{sym}").as_str()),
      Self::Kw(sym) => SmolStr::new_inline(format!("{sym}").as_str()),
      Self::Char(sym) => SmolStr::new_inline(format!("{sym}").as_str()),
      Self::Str(sym) => SmolStr::new_inline(format!("{sym}").as_str()),
    }
  }
}