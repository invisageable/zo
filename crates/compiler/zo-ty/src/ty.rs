//! ...

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

use hashbrown::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ty {
  pub kind: TyKind,
  pub span: Span,
}

impl Ty {
  pub const UNIT: Self = Self {
    kind: TyKind::Unit,
    span: Span::ZERO,
  };

  #[inline]
  pub const fn new(kind: TyKind, span: Span) -> Self {
    Self { kind, span }
  }

  #[inline]
  pub fn var(var: usize, span: Span) -> Self {
    Self::new(TyKind::Var(var), span)
  }

  #[inline]
  pub fn is(&self, kind: TyKind) -> bool {
    self.kind.is(kind)
  }

  #[inline]
  pub const fn is_numeric(&self) -> bool {
    self.kind.is_numeric()
  }

  pub fn ty_vars(&self) -> HashSet<usize> {
    self.kind.ty_vars()
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TyKind {
  /// unit — `()`.
  Unit,
  /// infer — `:=`.
  Infer,
  /// integer — `int`.
  Int,
  /// float — `float`.
  Float,
  /// identifier — `foo`, `Bar`, `foo_bar`, `BAR_FOO`.
  Ident(Symbol),
  /// boolean — `bool`.
  Bool,
  /// character — `ch`.
  Char,
  /// string — `str`.
  Str,
  /// variable.
  Var(usize),
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

  pub fn ty_vars(&self) -> HashSet<usize> {
    match self {
      Self::Unit => todo!(),
      Self::Var(var) => {
        let mut ty_vars = HashSet::new();

        ty_vars.insert(*var);

        ty_vars
      }
      _ => todo!(),
    }
  }
}
