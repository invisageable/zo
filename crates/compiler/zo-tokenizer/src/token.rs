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

/// The representation of a token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Token {
  /// The current token kind — see also [`TokenKind`].
  pub kind: TokenKind,
  /// The span of the current token — see also [`Span`].
  pub span: Span,
}

impl Token {
  /// A constant, it is used as a placeholder.
  pub const EOF: Self = Self::new(TokenKind::Eof, Span::ZERO);

  /// Creates a new token instance from a token kind and a span.
  #[inline]
  pub const fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }

  /// Check if the kind of a token matched from a other token kind.
  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    self.kind.is(kind)
  }
}

impl std::fmt::Display for Token {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.kind)
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
  Int(Symbol, int::Base),
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

  /// Checks if the token kind is a literal.
  #[inline]
  pub fn is_lit(&self) -> bool {
    matches!(
      self,
      Self::Int(..)
        | Self::Float(..)
        | Self::Ident(..)
        | Self::Kw(Kw::False)
        | Self::Kw(Kw::True)
        | Self::Char(..)
        | Self::Str(..)
    )
  }

  /// Checks if the token kind is a unary operator.
  #[inline]
  pub fn is_unop(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Exclamation)
        | Self::Punctuation(Punctuation::Minus)
    )
  }

  /// Checks if the token kind is a binary operator.
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

  /// Checks if the token kind is a group open.
  #[inline]
  pub fn is_group(&self) -> bool {
    matches!(
      self,
      Self::Group(Group::ParenOpen)
        | Self::Group(Group::BraceOpen)
        | Self::Group(Group::BracketOpen)
    )
  }

  /// Checks if the token kind is a keyword.
  #[inline]
  pub fn is_kw(&self) -> bool {
    matches!(self, Self::Kw(..))
  }

  /// Checks if the token kind is an assignment.
  #[inline]
  pub fn is_assignment(&self) -> bool {
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

  /// Checks if the token kind is a conditional.
  #[inline]
  pub fn is_conditional(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::AmpersandAmpersand)
        | Self::Punctuation(Punctuation::PipePipe)
    )
  }

  /// Checks if the token kind is a comparison.
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

  /// Checks if the token kind is a sum.
  #[inline]
  pub fn is_sum(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Plus)
        | Self::Punctuation(Punctuation::Minus)
    )
  }

  /// Checks if the token kind is a exponent.
  #[inline]
  pub fn is_exponent(&self) -> bool {
    matches!(
      self,
      Self::Punctuation(Punctuation::Asterisk)
        | Self::Punctuation(Punctuation::Slash)
        | Self::Punctuation(Punctuation::Percent)
    )
  }

  /// Checks if the token kind is a call function.
  #[inline]
  pub fn is_calling(&self) -> bool {
    matches!(self, Self::Group(Group::ParenOpen))
  }

  /// Checks if the token kind is an index.
  #[inline]
  pub fn is_index(&self) -> bool {
    matches!(self, Self::Group(Group::BracketOpen))
  }

  /// Checks if the token kind is a chaining.
  #[inline]
  pub fn is_chaining(&self) -> bool {
    matches!(self, Self::Punctuation(Punctuation::Period))
  }

  /// Checks if the token kind is a range.
  #[inline]
  pub fn is_range(&self) -> bool {
    matches!(self, Self::Punctuation(Punctuation::PeriodPeriod))
  }

  /// Checks if the token kind is an item.
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

  /// Checks if the token kind is a local variable.
  #[inline]
  pub fn is_var_local(&self) -> bool {
    matches!(self, Self::Kw(Kw::Imu) | Self::Kw(Kw::Mut))
  }
}

impl std::fmt::Display for TokenKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Eof => write!(f, "EOF"),
      Self::Unknown => write!(f, "UNKNOWN"),
      Self::Space => write!(f, "SPACE"),
      Self::Eol => write!(f, "EOL"),
      Self::Int(sym, base) => {
        write!(f, "{sym}--base--{base:?}")
      }
      Self::Float(sym) => write!(f, "{sym}"),
      Self::Punctuation(punctuation) => {
        write!(f, "{punctuation}")
      }
      Self::Group(group) => write!(f, "{group}"),
      Self::Ident(sym) => write!(f, "{sym}"),
      Self::Kw(sym) => write!(f, "{sym}"),
      Self::Char(sym) => write!(f, "{sym}"),
      Self::Str(sym) => write!(f, "{sym}"),
    }
  }
}
