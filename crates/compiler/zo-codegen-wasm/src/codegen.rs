use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::session::Session;

/// The representation of `wasm` code generation.
struct Codegen;
impl Codegen {
  fn generate(&self, session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
    Ok(vec![].into_boxed_slice())
  }
}

/// Transform an AST into bytecode.
#[inline]
pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}