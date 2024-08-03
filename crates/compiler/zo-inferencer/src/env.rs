use super::scheme::Scheme;

use hashbrown::HashSet;
use smol_str::SmolStr;

/// The representation of an environment for the type system.
#[derive(Clone, Debug)]
pub enum Env {
  /// An empty environment.
  Empty,
  /// A frame environment.
  Frame(SmolStr, Scheme, Box<Self>),
}

impl Env {
  /// Creates an empty environment.
  #[inline]
  pub fn empty() -> Self {
    Self::Empty
  }

  /// Creates an frame environment from a scheme.
  #[inline]
  pub fn extend(&self, ident: SmolStr, scheme: Scheme) -> Self {
    Self::Frame(ident, scheme, Box::new(self.to_owned()))
  }

  /// Gets a specific scheme from an identifier.
  pub fn get(&self, ident: &str) -> Option<Scheme> {
    match self {
      Self::Empty => None,
      Self::Frame(existing_ident, scheme, parent) => {
        if existing_ident == ident {
          Some(scheme.to_owned())
        } else {
          parent.get(ident)
        }
      }
    }
  }

  /// Gets the arguments type.
  pub fn ty_vars(&self) -> HashSet<usize> {
    match self {
      Self::Empty => HashSet::new(),
      Self::Frame(_, scheme, parent) => scheme
        .ty_vars()
        .union(&parent.ty_vars())
        .into_iter()
        .cloned()
        .collect(),
    }
  }
}
