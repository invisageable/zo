//! ...

use zhoo_ast::ast;
use zhoo_codegen_wasm as wasm;

use zhoo_session::backend::BackendKind;
use zhoo_session::session::Session;

use zo_core::Result;

#[derive(Debug)]
struct Codegen;

impl<'program> Codegen {
  /// no allocation.
  #[inline]
  fn generate(
    &mut self,
    session: &'program mut Session,
    program: &'program ast::Program,
  ) -> Result<Box<[u8]>> {
    match &session.settings.backend.kind {
      BackendKind::Wasm => wasm::codegen::generate(session, program),
      _ => unimplemented!(),
    }
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
