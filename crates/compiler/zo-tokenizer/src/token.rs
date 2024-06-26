//! ...

pub mod group;
pub mod int;
pub mod kw;
pub mod op;
pub mod punctuation;

use group::Group;
use kw::Kw;
use op::Op;
use punctuation::Punctuation;

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Token {
  pub kind: TokenKind,
  pub span: Span,
}

impl Token {
  #[inline]
  pub const fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }

  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    self.kind.is(kind)
  }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TokenKind {
  /// integer.
  Int(Symbol, int::BaseInt),
  /// float.
  Float(Symbol),
  /// operator.
  Op(op::Op),
  /// identifier.
  Ident(Symbol),
  /// keyword.
  Kw(kw::Kw),
  /// punctuation.
  Punctuation(punctuation::Punctuation),
  /// group.
  Group(group::Group),
  /// character.
  Char(Symbol),
  /// string.
  Str(Symbol),
  /// unknown.
  Unknwon,
  /// end of file.
  Eof,
}

impl TokenKind {
  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    *self == kind
  }

  #[inline]
  pub fn is_assignement(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::Equal)
        | Self::Op(Op::PlusEqual)
        | Self::Op(Op::MinusEqual)
        | Self::Op(Op::AsteriskEqual)
        | Self::Op(Op::SlashEqual)
        | Self::Op(Op::PercentEqual)
        | Self::Op(Op::CircumflexEqual)
        | Self::Op(Op::AmspersandEqual)
        | Self::Op(Op::PipeEqual)
        | Self::Op(Op::LessThanLessThanEqual)
        | Self::Op(Op::GreaterThanGreaterThanEqual)
    )
  }

  #[inline]
  pub fn is_conditional(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::AmpersandAmpersand) | Self::Op(Op::PipePipe)
    )
  }

  #[inline]
  pub fn is_comparison(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::EqualEqual)
        | Self::Op(Op::ExclamationEqual)
        | Self::Op(Op::LessThan)
        | Self::Op(Op::GreaterThan)
        | Self::Op(Op::LessThanEqual)
        | Self::Op(Op::GreaterThanEqual)
    )
  }

  #[inline]
  pub fn is_sum(&self) -> bool {
    matches!(self, Self::Op(Op::Plus) | Self::Op(Op::Minus))
  }

  #[inline]
  pub fn is_exponent(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::Asterisk) | Self::Op(Op::Slash) | Self::Op(Op::Percent)
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
    matches!(self, Self::Op(Op::Period))
  }

  #[inline]
  pub fn is_range(&self) -> bool {
    matches!(self, Self::Op(Op::PeriodPeriod))
  }

  #[inline]
  pub fn is_unop(&self) -> bool {
    matches!(self, Self::Op(Op::Exclamation) | Self::Op(Op::Minus))
  }

  #[inline]
  pub fn is_binop(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::Plus)
        | Self::Op(Op::Minus)
        | Self::Op(Op::Asterisk)
        | Self::Op(Op::Slash)
        | Self::Op(Op::Percent)
        | Self::Op(Op::Circumflex)
        | Self::Op(Op::EqualEqual)
        | Self::Op(Op::ExclamationEqual)
        | Self::Op(Op::LessThan)
        | Self::Op(Op::GreaterThan)
        | Self::Op(Op::LessThanEqual)
        | Self::Op(Op::GreaterThanEqual)
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
