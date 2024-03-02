//! ...

use zo_core::Result;

#[derive(Debug)]
struct Linker {}

impl Linker {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  fn link(&mut self) -> Result<()> {
    Ok(())
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn link() -> Result<()> {
  Linker::new().link()
}
