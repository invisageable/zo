use zo_core::Result;

#[derive(Debug)]
struct Analyzer {}

impl Analyzer {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  #[inline]
  fn analyze(&mut self) -> Result<()> {
    Ok(())
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn analyze() -> Result<()> {
  println!("analyze.");
  Analyzer::new().analyze()
}
