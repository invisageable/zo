//! ...

use super::env::Env;
use super::subst::Subst;
use super::supply::Supply;

use zo_ast::ast::Ast;
use zo_ty::ty::Ty;

use zo_core::Result;

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn infer(
  _env: &Env,
  _subst: &Subst,
  _supply: &mut Supply,
  _ast: &Ast,
  _ty: &Ty,
) -> Result<Subst> {
  todo!()
}
