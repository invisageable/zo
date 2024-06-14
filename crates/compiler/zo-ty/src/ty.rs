use zo_core::span::Span;

use hashbrown::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ty {
  pub kind: TyKind,
  pub span: Span,
}

impl Ty {
  pub const UNIT: Self = Self::new(TyKind::Unit, Span::ZERO);

  pub const fn new(kind: TyKind, span: Span) -> Self {
    Self { kind, span }
  }

  #[inline]
  pub fn var(var: usize, span: Span) -> Self {
    Self::new(TyKind::Var(var), span)
  }

  pub fn ty_vars(&self) -> HashSet<usize> {
    self.kind.ty_vars()
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TyKind {
  Unit,
  Var(usize),
}

impl TyKind {
  pub fn ty_vars(&self) -> HashSet<usize> {
    match self {
      Self::Unit => todo!(),
      Self::Var(var) => {
        let mut ty_vars = HashSet::new();

        ty_vars.insert(*var);

        ty_vars
      }
    }
  }
}
