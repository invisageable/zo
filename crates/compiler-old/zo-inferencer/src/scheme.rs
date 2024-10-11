use super::env::Env;
use super::supply::Supply;

use zo_ty::ty::Ty;

use hashbrown::HashSet;

/// The representation of a type scheme.
#[derive(Clone, Debug)]
pub struct Scheme(&'static Ty, HashSet<usize>);

impl Scheme {
  /// Creates a new scheme from an environment and the type to be generalised.
  #[inline]
  pub fn generalize(env: &Env, ty: &'static Ty) -> Self {
    Self(
      ty,
      ty.ty_vars().difference(&env.ty_vars()).cloned().collect(),
    )
  }

  /// Retrieves the set of quantified type variables in the type scheme.
  #[inline]
  pub fn ty_vars(&self) -> HashSet<usize> {
    self.1.to_owned()
  }
}
