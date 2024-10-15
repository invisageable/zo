use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::backend::Backend;
use zo_session::session::Session;

use zo_codegen_llvm as llvm;
use zo_codegen_py as py;
use zo_codegen_wasm as wasm;

/// The representation of code generation.
struct Codegen;
impl Codegen {
  /// Transform an AST into bytecode.
  fn generate(&self, session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
    match session.settings.backend {
      Backend::Llvm => llvm::codegen::generate(session, ast),
      Backend::Py => py::codegen::generate(session, ast),
      Backend::Wasm => wasm::codegen::generate(session, ast),
      _ => panic!(), // todo(ivs) — unsupported backend error message.
    }
  }
}

/// Transform an AST into bytecode — see also [`Codegen::generate`].
#[inline]
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
