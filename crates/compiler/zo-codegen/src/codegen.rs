use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::backend::Backend;
use zo_session::session::Session;

/// The representation of code generation.
struct Codegen;
impl Codegen {
  fn generate(&self, session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
    match session.settings.backend {
      Backend::Py => zo_codegen_py::codegen::generate(session, ast),
      Backend::Wasm => zo_codegen_wasm::codegen::generate(session, ast),
    }
  }
}

/// Transform an AST into bytecode.
#[inline]
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
