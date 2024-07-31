pub trait Symbolize {
  fn as_symbol(&self) -> &Symbol;
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Symbol(u32);

impl Symbol {
  /// Creates a new symbol from an index.
  #[inline]
  pub fn new(idx: u32) -> Self {
    Self(idx)
  }
}

impl std::fmt::Display for Symbol {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "${}", self.0)
  }
}

impl std::ops::Deref for Symbol {
  type Target = u32;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
