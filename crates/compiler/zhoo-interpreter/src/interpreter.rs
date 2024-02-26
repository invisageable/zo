use zo_core::Result;

#[derive(Debug)]
pub struct Interpreter {}

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

pub fn interpret() -> Result<()> {
  println!("interpret.");
  Interpreter::new().interpret()
}
