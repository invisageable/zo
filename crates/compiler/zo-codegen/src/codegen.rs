//! ...

use zo_ast::ast::Ast;
use zo_session::backend::BackendKind;
use zo_session::session::Session;

use zo_core::Result;

struct Codegen;

impl Codegen {
  fn generate(&self, session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
    match &session.settings.backend.kind {
      BackendKind::Py => zo_codegen_py::codegen::generate(session, ast),
      BackendKind::Wasm => zo_codegen_wasm::codegen::generate(session, ast),
    }
  }
}

/// ...
///
/// ## examples.
///
/// ```rs
/// ```
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
