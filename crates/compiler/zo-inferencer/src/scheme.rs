use super::env::Env;
use super::supply::Supply;

use zo_ty::ty::Ty;

use hashbrown::HashSet;

/// The representation of a scheme.
#[derive(Clone, Debug)]
pub struct Scheme(Ty, HashSet<usize>);

impl Scheme {
  /// Creates a new scheme.
  pub fn generalize(env: &Env, ty: &Ty) -> Self {
    Self(
      ty.clone(),
      ty.ty_vars()
        .difference(&env.ty_vars())
        .into_iter()
        .cloned()
        .collect(),
    )
  }

  /// ...
  pub fn ty_vars(&self) -> HashSet<usize> {
    self.1.to_owned()
  }
}
