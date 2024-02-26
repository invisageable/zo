use zo_core::Result;

#[derive(Debug)]
pub struct Analyzer {}

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

pub fn analyze() -> Result<()> {
  println!("analyze.");
  Analyzer::new().analyze()
}
