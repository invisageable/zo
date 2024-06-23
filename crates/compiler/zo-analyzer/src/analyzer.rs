//! ...

use zo_ast::ast::Ast;
use zo_checker::checker;
use zo_session::session::Session;

use zo_core::Result;

struct Analyzer;

impl Analyzer {
  fn analyze(&mut self, session: &mut Session, ast: &Ast) -> Result<()> {
    checker::name::check(session, ast)?;

    Ok(())
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn analyze(session: &mut Session, ast: &Ast) -> Result<()> {
  Analyzer.analyze(session, ast)
}
