use zo_ast::ast::Ast;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;

/// The representation of an inferencer.
struct Inferencer<'ast> {
  /// See [`Interner`].
  interner: &'ast mut Interner,
  /// See [`Reporter`].
  reporter: &'ast Reporter,
}

impl<'ast> Inferencer<'ast> {
  /// Creates a new inferencer.
  #[inline]
  pub fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  /// infers the AST from an AST.
  fn infer(&mut self, ast: &Ast) -> Result<Ast> {
    Ok(ast.to_owned())
  }
}

/// infers the AST from an AST.
pub fn infer(session: &mut Session, ast: &Ast) -> Result<Ast> {
  Inferencer::new(&mut session.interner, &session.reporter).infer(ast)
}
