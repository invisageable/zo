use zo_core::Result;

#[derive(Debug)]
pub struct Builder {}

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

pub fn build() -> Result<()> {
  println!("build.");
  Builder::new().build()
}
