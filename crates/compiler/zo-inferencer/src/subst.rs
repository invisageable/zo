use zo_ty::ty::{Ty, TyKind};

#[derive(Clone, Debug)]
pub enum Subst {
  Empty,
  Pair(Ty, Ty, Box<Self>),
}

impl Subst {
  /// Creates a new empty substitution.
  #[inline]
  pub fn empty() -> Self {
    Self::Empty
  }

  /// Creates a new extended substitution from two types.
  #[inline]
  pub fn extend(&self, from: Ty, to: Ty) -> Self {
    Self::Pair(from, to, Box::new(self.to_owned()))
  }

  /// Gets the type from a type.
  pub fn get(&self, ty: &Ty) -> Ty {
    match self {
      Self::Empty => ty.to_owned(),
      Self::Pair(from, to, parent) => {
        if from == ty {
          to.to_owned()
        } else {
          parent.get(ty)
        }
      }
    }
  }

  /// ...
  pub fn apply(&self, ty: &Ty) -> Ty {
    match &ty.kind {
      TyKind::Con(ident, ty_vars) => Ty::new(
        TyKind::Con(
          ident.to_owned(),
          ty_vars.iter().map(|ty| self.apply(ty)).collect(),
        ),
        ty.span,
      ),
      _ => panic!(),
    }
  }
}
