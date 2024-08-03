use super::subst::Subst;

use zo_reporter::{error, Result};
use zo_ty::ty::{Ty, TyKind};

use smol_str::ToSmolStr;

/// Unifies two types.
pub fn unify(subst: &Subst, t1: &Ty, t2: &Ty) -> Result<Subst> {
  let t1 = subst.apply(t1);
  let t2 = subst.apply(t2);

  match (&t1.kind, &t2.kind) {
    (TyKind::Con(t1_ident, t1_ty_vars), TyKind::Con(t2_ident, t2_ty_vars))
      if t1_ident == t2_ident =>
    {
      t1_ty_vars
        .iter()
        .zip(t2_ty_vars)
        .fold(Ok(subst.clone()), |subst, (t1, t2)| {
          unify(&subst?, &t1, &t2)
        })
    }
    (_, _) => Err(error::semantic::mismatched_types(
      (t1.span, t1.to_smolstr()),
      (t2.span, t2.to_smolstr()),
    )),
  }
}
