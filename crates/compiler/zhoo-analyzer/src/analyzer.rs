//! ...

use zhoo_ast::ast::Program;
use zhoo_checker::checker;
use zhoo_hir::hir::Hir;
use zhoo_inferencer::inferencer;
use zhoo_session::session::Session;

use zhoo_tychecker::tychecker;
use zo_core::span::Span;
use zo_core::Result;

#[derive(Debug)]
struct Analyzer;

impl Analyzer {
  #[inline]
  fn analyze(
    &mut self,
    session: &mut Session,
    program: &Program,
  ) -> Result<Hir> {
    checker::entry::check(session, program)?;
    checker::name::check(session, program)?;

    let hir = inferencer::infer(session, program)?; // todo (ivs) — should be an hir return value?
    #[allow(clippy::let_unit_value)]
    let _ = tychecker::check(session)?;

    println!("\n{hir:?}\n");

    session.reporter.abort_if_has_errors();

    // todo (ivs) — tmp.
    Ok(Hir { span: Span::ZERO })
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn analyze(session: &mut Session, program: &Program) -> Result<Hir> {
  Analyzer.analyze(session, program)
}
