//! ...

pub mod comment;
pub mod group;
pub mod kw;
pub mod op;
pub mod punctuation;

use group::Group;
use kw::Kw;
use op::Op;
use punctuation::Punctuation;

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::span::Span;

#[derive(Clone, Debug)]
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
  Int(Symbol),
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

  pub fn is_sum(&self) -> bool {
    matches!(self, Self::Op(Op::Plus) | Self::Op(Op::Minus))
  }

  pub fn is_exponent(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::Asterisk) | Self::Op(Op::Slash) | Self::Op(Op::Percent)
    )
  }

  pub fn is_conditional(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::AmpersandAmpersand) | Self::Op(Op::PipePipe)
    )
  }

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

  pub fn is_calling(&self) -> bool {
    matches!(self, Self::Group(Group::ParenOpen))
  }

  pub fn is_index(&self) -> bool {
    matches!(self, Self::Group(Group::BracketOpen))
  }

  pub fn is_chaining(&self) -> bool {
    matches!(self, Self::Punctuation(Punctuation::Period))
  }

  pub fn is_item(&self) -> bool {
    matches!(self, Self::Kw(Kw::Val) | Self::Kw(Kw::Fun))
  }

  pub fn is_stmt(&self) -> bool {
    matches!(self, |Self::Kw(Kw::Imu)| Self::Kw(Kw::Mut)
      | Self::Kw(Kw::Fun))
  }

  pub fn is_var(&self) -> bool {
    matches!(
      self,
      Self::Kw(Kw::Imu) | Self::Kw(Kw::Mut) | Self::Kw(Kw::Val)
    )
  }

  pub fn is_var_global(&self) -> bool {
    matches!(self, Self::Kw(Kw::Val))
  }

  pub fn is_var_local(&self) -> bool {
    matches!(self, Self::Kw(Kw::Imu) | Self::Kw(Kw::Mut))
  }

  pub fn is_lit(&self) -> bool {
    matches!(
      self,
      Self::Int(_)
        | Self::Float(_)
        | Self::Char(_)
        | Self::Str(_)
        | Self::Ident(_)
    )
  }

  pub fn is_unop(&self) -> bool {
    matches!(self, Self::Op(Op::Exclamation) | Self::Op(Op::Minus))
  }

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
      Self::Int(symbol) => symbol,
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
      Self::Int(int) => write!(f, "{int}"),
      Self::Float(float) => write!(f, "{float}"),
      Self::Ident(ident) => write!(f, "{ident}"),
      Self::Char(ch) => write!(f, "{ch}"),
      Self::Str(string) => write!(f, "{string}"),
      Self::Kw(kw) => write!(f, "{kw}"),
      Self::Op(op) => write!(f, "{op}"),
      Self::Group(group) => write!(f, "{group}"),
      Self::Punctuation(punctuation) => write!(f, "{punctuation}"),
      Self::Unknwon => write!(f, "UNKNOWN"),
      Self::Eof => write!(f, "EOF"),
    }
  }
}
