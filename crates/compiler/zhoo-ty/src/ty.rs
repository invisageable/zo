//! ...

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Ty {
  pub kind: TyKind,
  pub span: Span,
}

impl Ty {
  pub const UNIT: Self = Self::of(TyKind::Int, Span::ZERO);

  pub const fn of(kind: TyKind, span: Span) -> Self {
    Self { kind, span }
  }

  pub const fn unit(span: Span) -> Self {
    Self::of(TyKind::Unit, span)
  }

  pub const fn int(span: Span) -> Self {
    Self::of(TyKind::Int, span)
  }

  pub const fn float(span: Span) -> Self {
    Self::of(TyKind::Float, span)
  }

  pub const fn ident(ident: Symbol, span: Span) -> Self {
    Self::of(TyKind::Ident(ident), span)
  }

  pub const fn bool(span: Span) -> Self {
    Self::of(TyKind::Bool, span)
  }

  pub const fn char(span: Span) -> Self {
    Self::of(TyKind::Char, span)
  }

  pub const fn str(span: Span) -> Self {
    Self::of(TyKind::Str, span)
  }

  pub const fn fun(span: Span) -> Self {
    Self::of(TyKind::Fun, span)
  }

  pub const fn infer(span: Span) -> Self {
    Self::of(TyKind::Infer, span)
  }

  pub const fn custom(ident: Symbol, span: Span) -> Self {
    Self::of(TyKind::Custom(ident), span)
  }

  pub const fn struct_expr(props: Vec<(Symbol, Box<Ty>)>, span: Span) -> Self {
    Self::of(TyKind::StructExpr(props), span)
  }
}

impl Ty {
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
      TyKind::Ident(ident) => Ty::ident(*ident, ty.span),
      TyKind::Bool => Ty::bool(ty.span),
      TyKind::Char => Ty::char(ty.span),
      TyKind::Str => Ty::str(ty.span),
      TyKind::Fun => Ty::fun(ty.span),
      TyKind::Infer => Ty::infer(ty.span),
      TyKind::Custom(ident) => Ty::custom(*ident, ty.span),
      TyKind::StructExpr(props) => Ty::struct_expr(props.clone(), ty.span),
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
      _ => Ty::custom(find_me(ident), span),
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
  Fun,
  Infer,
  Custom(Symbol),
  StructExpr(Vec<(Symbol, Box<Ty>)>),
}

impl TyKind {
  pub fn is(&self, kind: TyKind) -> bool {
    *self == kind
  }
}

// todo (ivs) — find the related symbol from the interner symbol table.
// it should be implemented on the `parser` side.
fn find_me(_ident: &str) -> Symbol {
  todo!()
}
