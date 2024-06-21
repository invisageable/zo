//! ...

use super::translator::Translator;

use zo_ast::ast::Ast;
use zo_session::session::Session;

use zo_core::Result;

struct Codegen;

impl Codegen {
  fn generate(
    &mut self,
    session: &mut Session,
    ast: &Ast,
  ) -> Result<Box<[u8]>> {
    let mut translator = Translator::new(&session.interner, &session.reporter);

    translator.translate(ast)?;
    translator.output()
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
