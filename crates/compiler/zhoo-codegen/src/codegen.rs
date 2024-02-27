use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::Result;

#[derive(Debug)]
struct Codegen {}

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

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn generate(
  _session: &mut Session,
  _program: ast::Program,
) -> Result<Box<[u8]>> {
  println!("generate.");
  Codegen::new().generate()
}
