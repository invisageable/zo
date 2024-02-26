use zo_core::Result;

#[derive(Debug)]
pub struct Linker {}

impl Linker {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  #[inline]
  fn link(&mut self) -> Result<()> {
    Ok(())
  }
}

pub fn link() -> Result<()> {
  println!("link.");
  Linker::new().link()
}
