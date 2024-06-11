//! ...

pub mod group;
pub mod int;
pub mod kw;
pub mod op;
pub mod punctuation;

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

#[derive(Copy, Clone, Debug)]
pub struct Token {
  pub kind: TokenKind,
  pub span: Span,
}

impl Token {
  #[inline]
  pub fn new(kind: TokenKind, span: Span) -> Self {
    Self { kind, span }
  }
}

#[derive(Copy, Clone, Debug)]
pub enum TokenKind {
  Int(Symbol, int::BaseInt),
  Float(Symbol),
  Op(op::Op),
  Ident(Symbol),
  Kw(kw::Kw),
  Punctuation(punctuation::Punctuation),
  Group(group::Group),
  Char(Symbol),
  Str(Symbol),
  Unknwon,
  Eof,
}

impl TokenKind {
  #[inline]
  pub fn is_assignement(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_conditional(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_comparison(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_sum(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_exponent(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_calling(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_index(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_chaining(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_unop(&self) -> bool {
    todo!()
  }

  #[inline]
  pub fn is_binop(&self) -> bool {
    todo!()
  }
}
