//! ...

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

pub trait AsTy: Sized {
  fn as_ty(&self) -> Ty;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ty {
  pub kind: TyKind,
  pub span: Span,
}

impl Ty {
  pub const UNIT: Self = Self::of(TyKind::Unit, Span::ZERO);

  #[inline]
  pub const fn of(kind: TyKind, span: Span) -> Self {
    Self { kind, span }
  }

  #[inline]
  pub const fn unit(span: Span) -> Self {
    Self::of(TyKind::Unit, span)
  }

  #[inline]
  pub const fn int(span: Span) -> Self {
    Self::of(TyKind::Int, span)
  }

  #[inline]
  pub const fn float(span: Span) -> Self {
    Self::of(TyKind::Float, span)
  }

  #[inline]
  pub const fn ident(symbol: Symbol, span: Span) -> Self {
    Self::of(TyKind::Ident(symbol), span)
  }

  #[inline]
  pub const fn bool(span: Span) -> Self {
    Self::of(TyKind::Bool, span)
  }

  #[inline]
  pub const fn char(span: Span) -> Self {
    Self::of(TyKind::Char, span)
  }

  #[inline]
  pub const fn str(span: Span) -> Self {
    Self::of(TyKind::Str, span)
  }

  #[inline]
  pub const fn fun(ty: (Box<Ty>, Vec<Ty>), span: Span) -> Self {
    Self::of(TyKind::Fun(ty), span)
  }

  #[inline]
  pub const fn infer(span: Span) -> Self {
    Self::of(TyKind::Infer, span)
  }

  #[inline]
  pub const fn alias(symbol: Symbol, span: Span) -> Self {
    Self::of(TyKind::Alias(symbol), span)
  }

  #[inline]
  pub const fn pointer(ty: Box<Ty>, span: Span) -> Self {
    Self::of(TyKind::Pointer(ty), span)
  }

  #[inline]
  pub const fn array(ty: Box<Ty>, span: Span) -> Self {
    Self::of(TyKind::Array(ty), span)
  }

  #[inline]
  pub const fn struct_expr(symbol: Symbol, span: Span) -> Self {
    Self::of(TyKind::StructExpr(symbol), span)
  }

  #[inline]
  pub const fn is_numeric(&self) -> bool {
    self.kind.is_numeric()
  }
}

impl Ty {
  #[inline]
  pub fn is(&self, kind: TyKind) -> bool {
    self.kind.is(kind)
  }
}

impl From<&Ty> for Ty {
  fn from(ty: &Ty) -> Self {
    match &ty.kind {
      TyKind::Unit => Ty::unit(ty.span),
      TyKind::Int => Ty::int(ty.span),
      TyKind::Float => Ty::float(ty.span),
      TyKind::Ident(symbol) => Ty::ident(*symbol, ty.span),
      TyKind::Bool => Ty::bool(ty.span),
      TyKind::Char => Ty::char(ty.span),
      TyKind::Str => Ty::str(ty.span),
      TyKind::Fun((ty, tys)) => {
        Ty::fun((ty.to_owned(), tys.to_owned()), ty.span)
      }
      TyKind::Infer => Ty::infer(ty.span),
      TyKind::Alias(symbol) => Ty::alias(*symbol, ty.span),
      TyKind::Pointer(ty) => Ty::pointer(ty.to_owned(), ty.span),
      TyKind::Array(ty) => Ty::array(ty.to_owned(), ty.span),
      TyKind::StructExpr(symbol) => Ty::struct_expr(*symbol, ty.span),
    }
  }
}

impl From<(&str, Span)> for Ty {
  fn from((ident, span): (&str, Span)) -> Self {
    match ident {
      "int" => Ty::int(span),
      "float" => Ty::float(span),
      "bool" => Ty::bool(span),
      "char" => Ty::char(span),
      "str" => Ty::str(span),
      _ => Ty::alias(find_me(ident), span),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TyKind {
  Unit,
  Int,
  Float,
  Ident(Symbol),
  Bool,
  Char,
  Str,
  Fun((Box<Ty>, Vec<Ty>)),
  Infer,
  Alias(Symbol),
  Pointer(Box<Ty>),
  Array(Box<Ty>),
  StructExpr(Symbol),
}

impl TyKind {
  #[inline]
  pub fn is(&self, kind: TyKind) -> bool {
    *self == kind
  }

  #[inline]
  pub const fn is_numeric(&self) -> bool {
    matches!(self, Self::Int | Self::Float)
  }
}

// todo (ivs) — find the related symbol from the interner symbol table.
// it should be implemented on the `parser` side.
fn find_me(_ident: &str) -> Symbol {
  todo!()
}
