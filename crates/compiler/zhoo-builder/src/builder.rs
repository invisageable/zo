use zo_core::Result;

#[derive(Debug)]
struct Builder {}

impl Builder {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  #[inline]
  fn build(&mut self) -> Result<()> {
    Ok(())
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn build() -> Result<()> {
  println!("build.");
  Builder::new().build()
}
