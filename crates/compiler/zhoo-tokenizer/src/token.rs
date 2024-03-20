//! ...

pub mod comment;
pub mod group;
pub mod int;
pub mod kw;
pub mod op;
pub mod punctuation;

use group::Group;
use int::BaseInt;
use kw::Kw;
use op::Op;
use punctuation::Punctuation;

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
  pub kind: TokenKind,
  pub span: Span,
}

impl Token {
  #[inline]
  pub fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }

  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    self.kind.is(kind)
  }
}

impl Symbolize for Token {
  fn symbolize(&self) -> &Symbol {
    self.kind.symbolize()
  }
}

impl std::fmt::Display for Token {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.kind)
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TokenKind {
  Int(Symbol, BaseInt),
  Float(Symbol),
  Op(Op),
  Ident(Symbol),
  Kw(Kw),
  Punctuation(Punctuation),
  Group(Group),
  Char(Symbol),
  Str(Symbol),
  Unknwon,
  Eof,
}

impl TokenKind {
  #[inline]
  pub fn is(&self, kind: TokenKind) -> bool {
    *self == kind
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
  pub fn is_item(&self) -> bool {
    matches!(self, Self::Kw(Kw::Val) | Self::Kw(Kw::Fun))
  }

  #[inline]
  pub fn is_stmt(&self) -> bool {
    matches!(self, |Self::Kw(Kw::Imu)| Self::Kw(Kw::Mut)
      | Self::Kw(Kw::Fun))
  }

  #[inline]
  pub fn is_var(&self) -> bool {
    matches!(
      self,
      Self::Kw(Kw::Imu) | Self::Kw(Kw::Mut) | Self::Kw(Kw::Val)
    )
  }

  #[inline]
  pub fn is_var_global(&self) -> bool {
    matches!(self, Self::Kw(Kw::Val))
  }

  #[inline]
  pub fn is_var_local(&self) -> bool {
    matches!(self, Self::Kw(Kw::Imu) | Self::Kw(Kw::Mut))
  }

  #[inline]
  pub fn is_lit(&self) -> bool {
    matches!(
      self,
      Self::Int(_, _)
        | Self::Float(_)
        | Self::Char(_)
        | Self::Str(_)
        | Self::Ident(_)
    )
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
        // ...
        | Self::Op(Op::EqualEqual)
        | Self::Op(Op::ExclamationEqual)
        | Self::Op(Op::LessThan)
        | Self::Op(Op::GreaterThan)
        | Self::Op(Op::LessThanEqual)
        | Self::Op(Op::GreaterThanEqual)
    )
  }
}

impl Symbolize for TokenKind {
  fn symbolize(&self) -> &Symbol {
    match self {
      Self::Int(symbol, _) => symbol,
      Self::Float(symbol) => symbol,
      Self::Ident(symbol) => symbol,
      Self::Char(symbol) => symbol,
      Self::Str(symbol) => symbol,
      _ => unreachable!(),
    }
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
