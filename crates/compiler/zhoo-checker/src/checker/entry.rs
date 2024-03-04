//! ...

use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::interner::symbol::Symbolize;
use zo_core::interner::Interner;
use zo_core::reporter::report::semantic::Semantic;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::AsSpan;
use zo_core::Result;

const ENTRY_NAME: &str = "main";

#[derive(Debug)]
struct EntryChecker<'program> {
  interner: &'program mut Interner,
  reporter: &'program Reporter,
}

impl<'program> EntryChecker<'program> {
  fn new(
    interner: &'program mut Interner,
    reporter: &'program Reporter,
  ) -> Self {
    Self { interner, reporter }
  }

  fn check(&mut self, program: &'program ast::Program) -> Result<()> {
    let _maybe_entry = program.items.iter().find(|item| {
      if let ast::ItemKind::Fun(fun) = &item.kind {
        let pattern = &fun.prototype.pattern;
        let ident = self.interner.lookup_ident(*pattern.symbolize());

        if ident == ENTRY_NAME {
          if !fun.prototype.inputs.is_empty() {
            self.reporter.add_report(ReportError::Semantic(
              Semantic::NotFoundEntry(
                fun.prototype.inputs.as_span(),
                ident.into(),
              ),
            ));
          }

          return true;
        }
      }

      false
    });

    self.reporter.abort_if_has_errors();

    Ok(())
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn check(session: &mut Session, program: &ast::Program) -> Result<()> {
  EntryChecker::new(&mut session.interner, &session.reporter).check(program)
}
