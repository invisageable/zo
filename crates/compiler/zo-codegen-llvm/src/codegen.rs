use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::session::Session;

/// The representation of `llvm` code generation.
struct Codegen;
impl Codegen {
  /// Transform an AST into bytecode.
  fn generate(&self, _session: &mut Session, _ast: &Ast) -> Result<Box<[u8]>> {
    Ok(vec![].into_boxed_slice())
  }
}

/// Transform an AST into bytecode — see also [`Codegen::generate`].
#[inline]
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
