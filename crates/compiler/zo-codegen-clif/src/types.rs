//! `TyId` → Cranelift `ir::Type` mapping.
//!
//! Source of truth: `PLAN_CODEGEN_CLIF.md` Appendix B,
//! verified against `zo-ty-checker/src/tychecker.rs:96–151`.
//! TyIds 0–20 are pre-interned at startup.

use zo_ty::TyId;

use cranelift::codegen::ir;
use cranelift_module::Module;
use cranelift_object::ObjectModule;

/// Pointer-width CLIF type for the target. `I64` on every
/// current target; `I32` reserved for a future 32-bit path.
pub(crate) fn pointer_ty(module: &ObjectModule) -> ir::Type {
  module.target_config().pointer_type()
}

/// Maps a `TyId` to a Cranelift `ir::Type`. The mapping is
/// shallow: any pointer-shaped / aggregate type collapses to
/// `ptr` since CLIF scalars can't hold multi-word values —
/// those live in `StackSlot`s and the associated `Value` is a
/// pointer to the slot.
pub(crate) fn ty_id_to_clif(ty_id: TyId, ptr: ir::Type) -> ir::Type {
  match ty_id.0 {
    // Error, Unit — never emitted as a scalar. Caller must
    // skip or route through unit-handling (no returns vec,
    // etc.).
    0 | 1 => ptr,

    // Bool — canonical 0/1 in I8.
    2 => ir::types::I8,

    // Char — UTF-32 scalar.
    3 => ir::types::I32,

    // Str, Bytes — pointer to header.
    4 | 5 => ptr,

    // Signed integers (TyId 6..=10).
    6 => ir::types::I8,
    7 => ir::types::I16,
    8 => ir::types::I32,
    9 => ir::types::I64,
    10 => ptr, // Arch — pointer width on a 64-bit target.

    // Unsigned integers (TyId 11..=14) — signedness lives on
    // the op, not the CLIF type.
    11 => ir::types::I8,
    12 => ir::types::I16,
    13 => ir::types::I32,
    14 => ir::types::I64,

    // Floats.
    15 => ir::types::F32,
    16 => ir::types::F64,
    17 => ir::types::F64, // Arch — F64 on 64-bit.

    // 18+: Template / Type / Unknown are compile-time only
    // and must be resolved before codegen. 21+ are interned
    // aggregates — always pointer-sized.
    _ => ptr,
  }
}

/// Returns true iff the type is an unsigned integer
/// (`u8`/`u16`/`u32`/`u64`). Signed integers, floats, and
/// non-integers all return false. Used by `BinOp` dispatch to
/// pick `sdiv` vs `udiv`, `sshr` vs `ushr`, signed vs
/// unsigned `IntCC`s.
pub(crate) fn is_unsigned_int(ty_id: TyId) -> bool {
  matches!(ty_id.0, 11..=14)
}

/// Returns true iff the type is a float (`f32`/`f64`/`Arch`).
/// Used by `BinOp` dispatch to pick `fadd`/`fmul`/`fcmp` over
/// their integer counterparts.
pub(crate) fn is_float(ty_id: TyId) -> bool {
  matches!(ty_id.0, 15..=17)
}
