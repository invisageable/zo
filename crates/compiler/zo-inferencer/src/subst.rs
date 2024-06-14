//! ...

use zo_ty::ty::{Ty, TyKind};

#[derive(Clone, Debug)]
pub enum Subst {
  Empty,
  Pair(Ty, Ty, Box<Subst>),
}

impl Subst {
  #[inline]
  pub fn empty() -> Self {
    Self::Empty
  }

  #[inline]
  pub fn extend(&self, from: Ty, to: Ty) -> Self {
    Self::Pair(from, to, Box::new(self.clone()))
  }

  pub fn get(&self, ty: &Ty) -> Ty {
    match self {
      Self::Empty => ty.clone(),
      Self::Pair(from, to, parent) => {
        if from == ty {
          to.clone()
        } else {
          parent.get(ty)
        }
      }
    }
  }

  pub fn apply(&self, ty: &Ty) -> Ty {
    match &ty.kind {
      TyKind::Unit => todo!(),
      TyKind::Var(var) => {
        let to = self.get(&Ty::var(*var, ty.span));

        if ty == &to {
          to
        } else {
          self.apply(&to)
        }
      }
    }
  }
}
