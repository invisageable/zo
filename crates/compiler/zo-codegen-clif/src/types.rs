//! `TyId` → Cranelift `ir::Type` mapping.
//!
//! Phase 1: helper signatures only. Phase 2 fills the bodies
//! per `PLAN_CODEGEN_CRANELIFT.md` Appendix B.

use zo_ty::TyId;

use cranelift::codegen::ir;

/// Maps a `TyId` to a Cranelift `ir::Type`. Pointer-shaped
/// types (str, bytes, struct, enum, tuple, array, fn) fall
/// back to `pointer_ty` — typically `I64` on 64-bit targets.
///
/// Phase 1 stub returns `I64` — replaced in phase 2 with the
/// full Appendix B table.
#[allow(dead_code)]
pub(crate) fn ty_id_to_clif(_ty_id: TyId, pointer_ty: ir::Type) -> ir::Type {
  pointer_ty
}
