//! ...

use zo_core::Result;

#[derive(Debug)]
struct Interpreter {}

impl Interpreter {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  #[inline]
  fn interpret(&mut self) -> Result<()> {
    Ok(())
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn interpret() -> Result<()> {
  Interpreter::new().interpret()
}
