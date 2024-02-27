use serde::{Deserialize, Serialize};

pub trait Sym: std::fmt::Debug + std::fmt::Display {
  fn as_symbol(&self) -> &Symbol;
}

#[derive(
  Clone,
  Copy,
  Debug,
  Hash,
  Eq,
  PartialEq,
  PartialOrd,
  Ord,
  Deserialize,
  Serialize,
)]
pub struct Symbol(pub u32);

impl From<Symbol> for usize {
  fn from(symbol: Symbol) -> Self {
    symbol.0 as usize
  }
}

impl From<&Symbol> for u32 {
  fn from(symbol: &Symbol) -> Self {
    symbol.0
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
