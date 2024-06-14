//! ...

use super::env::Env;

use zo_ty::ty::Ty;

use hashbrown::HashSet;

#[derive(Clone, Debug)]
pub struct Scheme(pub Ty, pub HashSet<usize>);

impl Scheme {
  #[inline]
  pub fn new(ty: Ty) -> Self {
    Self(ty, HashSet::new())
  }

  #[inline]
  pub fn new_with_env(ty: Ty, vars: HashSet<usize>) -> Self {
    Self(ty, vars)
  }

  #[inline]
  pub fn generalize(env: &Env, ty: &Ty) -> Self {
    Self(
      ty.clone(),
      ty.ty_vars().difference(&env.ty_vars()).cloned().collect(),
    )
  }

  pub fn ty_vars(&self) -> HashSet<usize> {
    self.1.clone()
  }
}
