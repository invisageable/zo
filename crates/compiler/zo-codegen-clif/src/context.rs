//! Module + per-function translation context.
//!
//! The data threaded through every CLIF emission site: per-
//! module state ([`TCtx`]) borrowed from `translate_module`
//! for the duration of one body, and per-function state
//! ([`FunCtx`]) rebuilt from scratch for every SIR `FunDef`.
//! Constants for the uniform aggregate-slot layout live here
//! too since every consumer — translator, intrinsics, runtime
//! emitters — reads them.

use zo_interner::{Interner, Symbol};
use zo_ty::TyId;
use zo_value::ValueId;

use cranelift::codegen::ir;
use cranelift::frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataId, FuncId};
use cranelift_object::ObjectModule;
use rustc_hash::FxHashMap as HashMap;

/// Uniform slot size for every aggregate field / array element,
/// matching `zo-codegen-arm`'s ARM64 `STACK_SLOT_SIZE`. Pragmatic
/// shortcut: skip per-TyId layout computation by giving every
/// element 8 bytes, so `field_i` is at `base + i * 8` regardless
/// of element type. Wastes space for `u8` / `bool` / `f32`, but
/// keeps the codegen free of type-table lookups and identical
/// across backends.
pub(crate) const AGG_SLOT_SIZE: u32 = 8;

/// `log2(AGG_SLOT_SIZE)` — Cranelift's stack-slot alignment is
/// expressed as a shift, so `align = 1 << ALIGN_SHIFT`.
pub(crate) const AGG_ALIGN_SHIFT: u8 = 3;

/// Literal value of a module-scope `val X = ...;` (`ConstDef`).
/// Resolved once during `translate_module`'s declaration scan
/// and inlined at every `Load { Local(X) }` site — matches
/// plan row 6 ("Compile-time constant (inlined at uses)"). The
/// raw literal is stored so re-materialization at the use site
/// uses the same codepaths as a fresh literal (`ConstInt` →
/// `iconst`, `ConstString` → data section).
#[derive(Clone, Copy)]
pub(crate) enum ConstLiteral {
  /// Integer constant. `ty_id` determines the CLIF integer
  /// type at the use site.
  Int { value: u64, ty_id: TyId },
  /// Float constant. `ty_id` distinguishes `f32` (TyId 15)
  /// from `f64` / arch-float (TyId 16 / 17).
  Float { value: f64, ty_id: TyId },
  /// Boolean constant — always `I8` with value 0 or 1.
  Bool { value: bool },
  /// String literal — re-runs the `ConstString` data-section
  /// path with this `Symbol`; the per-symbol dedup map in
  /// `TCtx` ensures one data object per distinct literal.
  Str { symbol: Symbol },
}

/// Module-wide translation state threaded through
/// `translate_body`. Bundled so the signature stays manageable
/// as more per-module state accrues (string dedup, later:
/// array / aggregate data descriptors). Each field is borrowed
/// from `translate_module` for the duration of one body
/// translation — the struct itself is stack-allocated and
/// short-lived.
pub(crate) struct TCtx<'a> {
  /// The object module we're populating. `&mut` because
  /// `Insn::ConstString` calls `declare_anonymous_data` /
  /// `define_data`, and `Insn::Call` calls
  /// `declare_func_in_func`.
  pub(crate) module: &'a mut ObjectModule,
  /// String interner for `Symbol` → `&str` lookups
  /// (`Insn::ConstString` reads raw bytes here).
  pub(crate) interner: &'a Interner,
  /// Function `Symbol` → `FuncId`, populated in
  /// `translate_module`'s first pass.
  pub(crate) func_ids: &'a HashMap<Symbol, FuncId>,
  /// String literal `Symbol` → its allocated `DataId`. Dedupes
  /// repeated `ConstString` for the same symbol within one
  /// module — a single data object per distinct string.
  pub(crate) const_strings: &'a mut HashMap<Symbol, DataId>,
  /// Module-scope `val NAME = lit;` bindings (`ConstDef`).
  /// Populated in `translate_module`'s first pass, consulted
  /// at every `Load { Local(NAME) }` before the `FunCtx.vars`
  /// lookup.
  pub(crate) const_defs: &'a HashMap<Symbol, ConstLiteral>,
  /// Libc imports keyed by name (`"write"`, `"snprintf"`, …).
  /// Lazy: each entry is declared on first use via
  /// `ensure_libc_func`. The system linker resolves every
  /// symbol against libc / libSystem at link time.
  pub(crate) libc_funcs: &'a mut HashMap<&'static str, FuncId>,
  /// Module-scope anonymous `.rodata` objects keyed by a
  /// stable label (`"newline"`, `"fmt_int"`, `"true"`, …).
  /// Dedupes the small helper buffers used by
  /// `emit_io_intrinsic`.
  pub(crate) anon_data: &'a mut HashMap<&'static str, DataId>,
  /// Pointer-width CLIF type for the target. Cached so each
  /// `ConstString` doesn't re-query the module config.
  pub(crate) ptr_ty: ir::Type,
}

/// Per-function translation state. A fresh [`FunCtx`] is built
/// for every SIR `FunDef`.
pub(crate) struct FunCtx {
  /// SIR `ValueId` → CLIF `Value`.
  pub(crate) values: HashMap<ValueId, ir::Value>,
  /// SIR label id → CLIF `Block` (populated by the label
  /// pre-pass so forward jumps can resolve).
  pub(crate) blocks: HashMap<u32, ir::Block>,
  /// Symbol-keyed slots — locals declared by `Insn::VarDef`
  /// plus parameters mirrored under their declaration name so
  /// `Store { name }` resolves regardless of whether it hits
  /// a param or a local. Cranelift's `Variable` API bridges
  /// SIR's mutable named slots and CLIF's SSA: `def_var` for
  /// writes, `use_var` for reads, phi nodes auto-inserted.
  pub(crate) vars: HashMap<Symbol, Variable>,
  /// Parameters in declaration order. `Load { Param(idx) }`
  /// indexes this vec directly (SIR's `Param` carries a u32
  /// slot index, not a symbol, so name-keyed lookup can't
  /// serve it).
  pub(crate) params: Vec<Variable>,
  /// True iff the current CLIF block has a terminator already
  /// emitted (`return_`, `jump`, `brif`, `trap`). CLIF's
  /// `FunctionBuilder::is_filled` is private, so we mirror the
  /// state here — set by every terminator emission, reset on
  /// `switch_to_block`. Used at each `Label` to decide whether
  /// a fall-through jump needs synthesizing.
  pub(crate) terminated: bool,
  /// True iff the function being translated is `main`. Flips
  /// the `Return { value: None }` arm into emitting `iconst
  /// (I32, 0)` so the process exits with a deterministic `0`
  /// instead of a junk return-register value.
  pub(crate) is_main: bool,
  /// SIR `ValueId` → zo `TyId`. Populated incrementally as
  /// each value-producing insn is translated. Consulted by
  /// `emit_io_intrinsic` to dispatch `show` / `showln` per
  /// argument type (int vs bool vs str vs …). Mirrors
  /// `zo-codegen-arm`'s `value_types` pattern.
  pub(crate) value_types: HashMap<ValueId, TyId>,
}

impl FunCtx {
  pub(crate) fn new(is_main: bool) -> Self {
    Self {
      values: HashMap::default(),
      blocks: HashMap::default(),
      vars: HashMap::default(),
      params: Vec::new(),
      terminated: false,
      is_main,
      value_types: HashMap::default(),
    }
  }

  /// Declares a fresh [`Variable`] for `name` with CLIF type
  /// `ty` on the builder and records the mapping. Cranelift
  /// mints the `Variable` internally (`declare_var` returns
  /// it); we just thread it into `vars`. Called for every
  /// parameter at entry and every `VarDef`.
  pub(crate) fn declare_local(
    &mut self,
    builder: &mut FunctionBuilder,
    name: Symbol,
    ty: ir::Type,
  ) -> Variable {
    let var = builder.declare_var(ty);

    self.vars.insert(name, var);

    var
  }
}
