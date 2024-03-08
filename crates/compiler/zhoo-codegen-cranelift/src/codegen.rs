use super::translator::Translator;

use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::Result;

pub struct Codegen;

impl Codegen {
  pub fn generate(
    &mut self,
    session: &mut Session,
    program: &ast::Program,
  ) -> Result<Box<[u8]>> {
    let mut translator = Translator::new(&session.interner, &session.reporter);

    translator.translate(program)?;
    translator.output()
  }
}

pub fn generate(
  session: &mut Session,
  program: &ast::Program,
) -> Result<Box<[u8]>> {
  Codegen.generate(session, program)
}
