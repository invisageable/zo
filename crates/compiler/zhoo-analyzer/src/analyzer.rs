//! ...

use zhoo_ast::ast::Program;
use zhoo_checker::checker;
use zhoo_hir::hir::Hir;
use zhoo_inferencer::inferencer;
use zhoo_session::session::Session;
use zhoo_tychecker::tychecker;

use zo_core::Result;

#[derive(Debug)]
struct Analyzer;

impl Analyzer {
  fn analyze<'hir>(
    &mut self,
    session: &mut Session,
    program: &Program,
  ) -> Result<Hir<'hir>> {
    #[allow(clippy::let_unit_value)]
    let _ = checker::entry::check(session, program)?;
    #[allow(clippy::let_unit_value)]
    let _ = checker::name::check(session, program)?;
    let hir = inferencer::infer(session, program)?; // todo (ivs) — should be an hir return value?
    #[allow(clippy::let_unit_value)]
    let _ = tychecker::tycheck(session, program)?;

    println!("\n{hir:?}\n");

    session.reporter.abort_if_has_errors();

    // todo (ivs) — tmp.
    Ok(Hir {
      items: &[],
      span: program.span,
    })
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn analyze<'hir>(
  session: &mut Session,
  program: &Program,
) -> Result<Hir<'hir>> {
  Analyzer.analyze(session, program)
}
