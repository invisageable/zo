use serde::Serialize;

/// A Symbol is a compact, type-safe representation of an interned string.
/// It's just a u32 index into the interner's storage.
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize,
)]
pub struct Symbol(pub u32);
impl Symbol {
  /// Creates a new Symbol from a raw index.
  /// This is primarily for internal use by the Interner.
  #[inline(always)]
  pub const fn new(idx: u32) -> Self {
    Symbol(idx)
  }

  /// Returns the raw index of this symbol.
  #[inline(always)]
  pub const fn as_u32(self) -> u32 {
    self.0
  }
}
impl Symbol {
  pub const EMPTY: Symbol = Symbol(0);
  pub const UNDERSCORE: Symbol = Symbol(1);
  pub const FUN: Symbol = Symbol(2);
  pub const MUT: Symbol = Symbol(3);
  pub const IMU: Symbol = Symbol(4);
  pub const IF: Symbol = Symbol(5);
  pub const ELSE: Symbol = Symbol(6);
  pub const WHILE: Symbol = Symbol(7);
  pub const FOR: Symbol = Symbol(8);
  pub const RETURN: Symbol = Symbol(9);
  pub const BREAK: Symbol = Symbol(10);
  pub const CONTINUE: Symbol = Symbol(11);
  pub const MATCH: Symbol = Symbol(12);
  pub const WHEN: Symbol = Symbol(13);
  pub const AS: Symbol = Symbol(14);
  pub const IS: Symbol = Symbol(15);
  pub const TRUE: Symbol = Symbol(16);
  pub const FALSE: Symbol = Symbol(17);
  pub const SELF_UPPER: Symbol = Symbol(18);
  pub const SELF_LOWER: Symbol = Symbol(19);
  pub const STRUCT: Symbol = Symbol(20);
  pub const ENUM: Symbol = Symbol(21);
  pub const TYPE: Symbol = Symbol(22);
  pub const PUB: Symbol = Symbol(23);
  pub const VAL: Symbol = Symbol(24);
  pub const FIRST_DYNAMIC: u32 = 25;
}
impl std::ops::Deref for Symbol {
  type Target = u32;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl std::fmt::Display for Symbol {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}
