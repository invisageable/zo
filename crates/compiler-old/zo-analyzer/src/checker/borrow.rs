use zo_ast::ast;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;

/// The representation of a borrow checker.
struct BorrowChecker<'ast> {
  /// The interner — see also [`Interner`].
  interner: &'ast mut Interner,
  /// The reporter — see also [`Reporter`].
  reporter: &'ast Reporter,
}

impl<'ast> BorrowChecker<'ast> {
  /// Creates a new borrow checker.
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  /// Checks the borrowing from an AST.
  fn check(&mut self, _ast: &ast::Ast) -> Result<()> {
    Ok(())
  }
}

/// Checks the borrowing from an AST.
pub fn check(session: &mut Session, ast: &ast::Ast) -> Result<()> {
  BorrowChecker::new(&mut session.interner, &session.reporter).check(ast)
}
