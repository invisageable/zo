use smol_str::{SmolStr, ToSmolStr};

/// Adds a behavior to get the symbol of an instance which already had a symbol
/// as property.
pub trait Symbolize {
  /// Gets the symbol of an instance.
  fn as_symbol(&self) -> &Symbol;
}

/// The representation of a symbol.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Symbol(u32);

impl Symbol {
  /// Creates a new symbol from an index.
  #[inline(always)]
  pub fn new(idx: u32) -> Self {
    Self(idx)
  }
}

impl From<Symbol> for SmolStr {
  #[inline(always)]
  fn from(sym: Symbol) -> Self {
    sym.to_smolstr()
  }
}

impl std::fmt::Display for Symbol {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "${}", self.0)
  }
}

impl std::ops::Deref for Symbol {
  type Target = u32;

  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
