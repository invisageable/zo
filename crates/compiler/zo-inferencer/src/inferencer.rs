use super::env::Env;
use super::subst::Subst;
use super::supply::Supply;

use zo_ast::ast::Ast;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;

/// The representation of an inferencer.
struct Inferencer<'ast> {
  /// The environment — see also [`Env`].
  env: &'ast Env,
  /// The substitution — see also [`Subst`].
  subst: Subst,
  /// ...
  supply: Supply,
  /// See [`Interner`].
  interner: &'ast mut Interner,
  /// See [`Reporter`].
  reporter: &'ast Reporter,
}

impl<'ast> Inferencer<'ast> {
  /// Creates a new inferencer.
  #[inline]
  fn new(
    interner: &'ast mut Interner,
    reporter: &'ast Reporter,
    env: &'ast Env,
  ) -> Self {
    Self {
      env,
      subst: Subst::Empty,
      supply: Supply::new(),
      interner,
      reporter,
    }
  }

  /// infers the AST from an AST.
  fn infer(&mut self, ast: &Ast) -> Result<Ast> {
    for _stmt in ast.iter() {}

    Ok(ast.to_owned())
  }
}

/// infers the AST from an AST.
pub fn infer(session: &mut Session, ast: &Ast, env: &Env) -> Result<Ast> {
  Inferencer::new(&mut session.interner, &session.reporter, env).infer(ast)
}
