use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::Result;

pub struct Codegen;

impl Codegen {
  fn generate(
    &mut self,
    _session: &mut Session,
    _program: &ast::Program,
  ) -> Result<Box<[u8]>> {
    todo!()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn generate(
  session: &mut Session,
  program: &ast::Program,
) -> Result<Box<[u8]>> {
  Codegen.generate(session, program)
}
