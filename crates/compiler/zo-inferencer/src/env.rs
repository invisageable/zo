//! ...

use super::scheme::Scheme;

use hashbrown::HashSet;
use smol_str::SmolStr;

#[derive(Clone, Debug)]
pub enum Env {
  Empty,
  Frame(SmolStr, Scheme, Box<Self>),
}

impl Env {
  #[inline]
  pub fn empty() -> Self {
    Self::Empty
  }

  #[inline]
  pub fn extend(&self, ident: SmolStr, scheme: Scheme) -> Self {
    Self::Frame(ident, scheme, Box::new(self.clone()))
  }

  pub fn get(&self, ident: &SmolStr) -> Option<Scheme> {
    match self {
      Self::Empty => None,
      Self::Frame(existing_ident, scheme, parent) => {
        if existing_ident == ident {
          Some(scheme.clone())
        } else {
          parent.get(ident)
        }
      }
    }
  }

  pub fn ty_vars(&self) -> HashSet<usize> {
    match self {
      Self::Empty => HashSet::new(),
      Self::Frame(_, scheme, parent) => {
        scheme.ty_vars().union(&parent.ty_vars()).cloned().collect()
      }
    }
  }
}
