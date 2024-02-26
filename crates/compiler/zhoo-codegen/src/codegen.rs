use zo_core::Result;

#[derive(Debug)]
pub struct Codegen {}

impl Codegen {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  #[inline]
  fn generate(&mut self) -> Result<Box<[u8]>> {
    Ok(Vec::with_capacity(0usize).into_boxed_slice())
  }
}

pub fn generate() -> Result<Box<[u8]>> {
  println!("generate.");
  Codegen::new().generate()
}
