//! ```sh
//! cargo test -p zo-interner
//! ```

pub(crate) mod common;
pub(crate) mod errors;

use crate::interner::Interner;
use crate::symbol::Symbol;

#[test]
fn test_interner_basic() {
  let mut interner = Interner::new();

  assert_eq!(interner.get(Symbol::FUN), "fun");
  assert_eq!(interner.get(Symbol::IF), "if");

  let hello1 = interner.intern("hello");
  let world = interner.intern("world");
  let hello2 = interner.intern("hello");

  assert_eq!(hello1, hello2);
  assert_ne!(hello1, world);

  assert_eq!(interner.get(hello1), "hello");
  assert_eq!(interner.get(world), "world");
}
