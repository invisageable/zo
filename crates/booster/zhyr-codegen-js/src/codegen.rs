use super::translator::Translator;

use zhyr_ast::ast::Ast;

use zhoo_session::session::Session;

use zo_core::Result;

struct Codegen;

impl Codegen {
  fn generate(
    &mut self,
    _session: &mut Session,
    ast: &Ast,
  ) -> Result<Box<[u8]>> {
    let mut translator = Translator::new();

    translator.translate(ast)?;
    translator.output()
  }
}

pub fn generate(session: &mut Session, ast: &Ast) -> Result<Box<[u8]>> {
  Codegen.generate(session, ast)
}
