use zhyr_ast::ast::Ast;
use zhyr_codegen_js as js;
use zhyr_codegen_py as py;

use zhoo_session::backend::BackendKind;
use zhoo_session::session::Session;

use zo_core::Result;

struct Codegen;

impl Codegen {
  fn generate(
    &mut self,
    session: &mut Session,
    ast: &Ast,
  ) -> Result<Box<[u8]>> {
    match &session.settings.backend.kind {
      BackendKind::Js => js::codegen::generate(session, ast),
      BackendKind::Py => py::codegen::generate(session, ast),
      backend => panic!("backend `{backend} not supported.`"),
    }
  }
}

pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
