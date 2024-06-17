//! ...

use zo_ast::ast::Ast;
use zo_inferencer::env::Env;
use zo_ty::ty::Ty;

use zo_core::Result;

struct Tychecker<'ast> {
  env: &'ast Env,
}

impl<'ast> Tychecker<'ast> {
  #[inline]
  fn new(env: &'ast Env) -> Self {
    Self { env }
  }

  fn tycheck(&mut self, _ast: &Ast) -> Result<Ty> {
    todo!()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn tycheck(env: &Env, ast: &Ast) -> Result<Ty> {
  Tychecker::new(env).tycheck(ast)
}
