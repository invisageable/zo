//! ...

use zo_ast::ast::{Ast, ItemKind, Stmt, StmtKind};
use zo_session::session::Session;

use zo_core::interner::symbol::Symbolize;
use zo_core::interner::Interner;
use zo_core::reporter::report::semantic::Semantic;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::AsSpan;
use zo_core::Result;

const ENTRY_NAME: &str = "main";

struct EntryChecker<'ast> {
  interner: &'ast mut Interner,
  reporter: &'ast Reporter,
}

impl<'ast> EntryChecker<'ast> {
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  fn check(&mut self, ast: &Ast) -> Result<()> {
    if !ast.iter().any(|stmt| self.check_entry(stmt)) {
      self
        .reporter
        .add_report(ReportError::Semantic(Semantic::NotFoundEntry(
          ast.as_span(),
          ENTRY_NAME.to_string(),
        )))
    }

    self.reporter.abort_if_has_errors();

    Ok(())
  }

  fn check_entry(&mut self, stmt: &Stmt) -> bool {
    if let StmtKind::Item(item) = &stmt.kind {
      if let ItemKind::Fun(fun) = &item.kind {
        let entry_name = self
          .interner
          .lookup_ident(*fun.prototype.pattern.as_symbol());

        if entry_name == ENTRY_NAME {
          if !fun.prototype.inputs.is_empty() {
            self.reporter.add_report(ReportError::Semantic(
              Semantic::NoArgsEntry(fun.prototype.inputs.as_span()),
            ));
          }

          return true;
        }
      }
    }

    false
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn check(session: &mut Session, ast: &Ast) -> Result<()> {
  EntryChecker::new(&mut session.interner, &session.reporter).check(ast)
}
