pub mod comment;
pub mod group;
pub mod kw;
pub mod op;
pub mod punctuation;

use group::Group;
use kw::Kw;
use op::Op;
use punctuation::Punctuation;

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Token {
  pub kind: TokenKind,
  pub span: Span,
}

impl Token {
  pub fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }

  pub fn is(&self, kind: TokenKind) -> bool {
    self.kind.is(kind)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum TokenKind {
  Int(Symbol),
  Float(Symbol),
  Op(Op),
  Ident(Symbol),
  Kw(Kw),
  Punctuation(Punctuation),
  Group(Group),
  Char(Symbol),
  String(Symbol),
  Unknwon,
  Eof,
}

impl TokenKind {
  pub fn is(&self, kind: TokenKind) -> bool {
    *self == kind
  }

  pub fn is_lit(&self) -> bool {
    matches!(
      self,
      Self::Int(_)
        | Self::Float(_)
        | Self::Char(_)
        | Self::String(_)
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
    )
  }

  // pub fn is_comparison(&self) -> bool {
  //   matches!(
  //     self,
  //     Self::Op(Op::EqualEqual)
  //       | Self::Op(Op::ExclamationEqual)
  //       | Self::Op(Op::LessThan)
  //       | Self::Op(Op::GreaterThan)
  //       | Self::Op(Op::LessThanEqual)
  //       | Self::Op(Op::GreaterThanEqual)
  //   )
  // }

  pub fn is_sum(&self) -> bool {
    matches!(self, Self::Op(Op::Plus) | Self::Op(Op::Minus))
  }

  pub fn is_exponent(&self) -> bool {
    matches!(
      self,
      Self::Op(Op::Asterisk) | Self::Op(Op::Slash) | Self::Op(Op::Percent)
    )
  }

  pub fn is_stmt(&self) -> bool {
    matches!(
      self,
      Self::Kw(Kw::Val)
        | Self::Kw(Kw::Imu)
        | Self::Kw(Kw::Mut)
        | Self::Kw(Kw::Fun)
    )
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
}
