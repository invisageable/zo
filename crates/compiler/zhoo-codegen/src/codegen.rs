//! ...

use zhoo_ast::ast;
use zhoo_codegen_arm as arm;
use zhoo_codegen_clif as clif;
use zhoo_codegen_js as js;
use zhoo_codegen_llvm as llvm;
use zhoo_codegen_py as py;
use zhoo_codegen_wasm as wasm;

use zhoo_session::backend::BackendKind;
use zhoo_session::session::Session;

use zo_core::Result;

#[derive(Debug)]
struct Codegen;

impl<'program> Codegen {
  #[inline]
  fn generate(
    &mut self,
    session: &'program mut Session,
    program: &'program ast::Program,
  ) -> Result<Box<[u8]>> {
    match &session.settings.backend.kind {
      BackendKind::Arm => arm::codegen::generate(session, program),
      BackendKind::Clif => clif::codegen::generate(session, program),
      BackendKind::Js => js::codegen::generate(session, program),
      BackendKind::Llvm => llvm::codegen::generate(session, program),
      BackendKind::Py => py::codegen::generate(session, program),
      BackendKind::Wasm => wasm::codegen::generate(session, program),
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
