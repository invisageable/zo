use zo_core::Result;

#[derive(Debug)]
pub struct Codegen {}

pub fn generate() -> Result<Box<[u8]>> {
  println!("generate.");
  Ok(Vec::with_capacity(0usize).into_boxed_slice())
}
