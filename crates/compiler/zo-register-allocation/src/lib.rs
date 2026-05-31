pub mod allocator;

#[cfg(test)]
mod tests;

use zo_interner::{Interner, Symbol};
use zo_sir::Insn;
use zo_ty::{Ty, TyId, TyTable};
use zo_value::{FunctionKind, ValueId};

use rustc_hash::FxHashMap as HashMap;

// Re-export liveness utilities so existing consumers
// don't need to add zo-liveness directly.
pub use zo_liveness::{compute_value_ids, visit_uses};

/// Slots reserved per IO Result frame
/// (`Result tag + heap ptr + scratch`).
pub const IO_RESULT_FRAME_SLOTS: u32 = 3;

/// Slots reserved for the shared IO read buffer in any
/// function that calls `read_file` / `readln` / `read`.
/// Sized for the codegen's 4096-byte buffer plus an
/// 8-byte null/alignment slack — must match
/// `zo_codegen_arm::IO_SHARED_BUF_SLOTS`.
pub const IO_SHARED_BUF_SLOTS: u32 = 513;

/// Caller-saved GP register indices, preferred order.
/// Temps (X9-X15) first, then args (X1-X7). X0 is
/// RESERVED for call-result values — the Call handler
/// at line 334 hard-codes the callee's return as `reg=0`,
/// and the reload-after-call path reallocates any live
/// value that happened to be in X0 into a fresh register
/// (x0 now holds the call result). That reallocation
/// rewrites `assignments[vid]`, so the original def (e.g.
/// `ConstInt` emitted BEFORE the call) ends up targeting
/// the new register — while the already-emitted spill
/// store still references x0. Result: the spill reads
/// the callee's stale result instead of the real value.
/// Removing x0 from the pool keeps regular values out of
/// x0 entirely, so the reload path never needs to
/// reallocate. Bug surfaced by `3 + five() / 2` appearing
/// twice in a row (CL15).
pub const ALLOCATABLE_GP: [u8; 14] =
  [9, 10, 11, 12, 13, 14, 15, 1, 2, 3, 4, 5, 6, 7];

/// Caller-saved FP register indices, preferred order.
/// Temps (D16-D31) first, then args (D1-D7). D0 is
/// RESERVED for call-result values for the same reason
/// as X0 above.
pub const ALLOCATABLE_FP: [u8; 23] = [
  16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 1, 2, 3, 4,
  5, 6, 7,
];

/// GP vs FP register classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterClass {
  GP,
  FP,
}

/// When to emit a spill operation relative to an
/// instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitTiming {
  Before,
  After,
}

/// Per-function metadata produced by the allocator.
pub struct FunctionInfo {
  /// Whether the function contains any Call instructions.
  pub has_calls: bool,
  /// Number of spill slots used.
  pub spill_count: u32,
  /// Stack space for spills, aligned to 16.
  pub spill_size: u32,
  /// Total stack bytes for struct allocations in this
  /// function (sum of all StructConstruct field counts
  /// * 8, aligned to 16).
  pub struct_size: u32,
  /// Stack space for mutable variable slots: unique
  /// Store targets × 8, aligned to 16.
  pub mutable_size: u32,
  /// Scratch stack space for channel primitives —
  /// `ChannelSend` stores the value there before
  /// `zo_chan_send(chan, src)` reads it through its
  /// `src` pointer; `ChannelRecv` reserves the same
  /// slot for its output buffer. 16 bytes when the
  /// function contains any channel op, 0 otherwise.
  pub chan_scratch_size: u32,
  /// Scratch stack space for `SelectWait` — holds the
  /// on-stack `*const *mut ZoChan` array plus the
  /// `elem_sz`-byte output buffer that the runtime
  /// writes into. Sized at `max(nchans * 8 + elem_sz)`
  /// across every `SelectWait` in the function, aligned
  /// to 16. Zero when the function has no select.
  pub select_scratch_size: u32,
  /// Scratch stack space for `StringFormat` — holds the
  /// on-stack pointer array passed to `_zo_str_multi_concat`.
  /// Sized at `max(segments.len()) * 8` across every
  /// `StringFormat` in the function, aligned to 16.
  pub string_format_scratch_size: u32,
}

/// A spill operation to emit during codegen.
pub struct SpillOp {
  /// SIR instruction index this is associated with.
  pub insn_idx: usize,
  /// Emit before or after the instruction.
  pub timing: EmitTiming,
  /// The load or store to emit.
  pub kind: SpillKind,
}

/// Spill operation kind.
#[derive(Clone)]
pub enum SpillKind {
  /// STR reg, [SP, #slot*8]
  Store {
    reg: u8,
    slot: u32,
    class: RegisterClass,
  },
  /// LDR reg, [SP, #slot*8]
  Load {
    reg: u8,
    slot: u32,
    class: RegisterClass,
    vid: u32,
  },
}

/// The result of register allocation over the entire SIR.
pub struct RegAlloc {
  /// Flat GP map — fallback for values that the per-insn
  /// snapshot misses (liveness gaps, codegen-side lookups
  /// at non-standard instruction indices).
  pub assignments: HashMap<(u32, u32), u8>,
  /// Flat FP map (same role as `assignments`).
  pub fp_assignments: HashMap<(u32, u32), u8>,
  /// Per-instruction GP register state.
  /// `insn_gp[global_insn_idx]` maps `ValueId.0 → GP
  /// register` — the physical register holding that value
  /// when the codegen begins emitting instruction
  /// `global_insn_idx`. Primary authority for GP lookups.
  pub insn_gp: Vec<HashMap<u32, u8>>,
  /// Per-instruction FP register state (same semantics).
  pub insn_fp: Vec<HashMap<u32, u8>>,
  /// Spill operations emitted by the allocator.
  pub spill_ops: Vec<SpillOp>,
  /// ValueId produced by each instruction (parallel array).
  pub value_ids: Vec<Option<ValueId>>,
  /// Per-function info, keyed by function start index.
  pub function_info: HashMap<usize, FunctionInfo>,
  /// Function name → deep flat-slot count for every
  /// struct-returning function. Exposed so the codegen
  /// can reuse the map for its call-site deep-copy loop
  /// instead of re-scanning the SIR a second time.
  pub struct_return_fns: HashMap<Symbol, u32>,
  /// Function name → per-variant substituted struct
  /// payload field info, for enum-returning functions.
  /// Each entry's outer `Vec` is indexed by variant
  /// discriminant (0..N); each inner `Vec` lists
  /// `(field_index, struct_ty_id)` pairs for variant
  /// payload fields whose substituted type resolves to
  /// a struct. Codegen consumes this in the enum-deep-
  /// copy path because the enum type itself still carries
  /// unsubstituted generic placeholders (zo doesn't
  /// intern monomorphized enum TyIds).
  pub enum_payload_struct_fields: EnumPayloadFields,
}

/// `(field_index, substituted_struct_ty_id)` for one
/// variant payload field whose concrete type resolves
/// to a struct.
pub type EnumVariantStructFields = Vec<(u32, TyId)>;

/// Function name → list of [`EnumVariantStructFields`],
/// indexed by variant discriminant. See
/// [`RegAlloc::enum_payload_struct_fields`] for the why.
pub type EnumPayloadFields = HashMap<Symbol, Vec<EnumVariantStructFields>>;

/// Inputs to [`RegAlloc::allocate`], bundled so the entry
/// point takes one argument rather than a long borrow list.
pub struct AllocInput<'a> {
  /// Whole-program SIR instruction stream.
  pub insns: &'a [Insn],
  /// Total `ValueId` count — sizes liveness bitsets.
  pub next_value_id: u32,
  /// Interner for resolving `Call` / runtime symbol names.
  pub interner: &'a Interner,
  /// Type tables, when available (ARM path). Drives
  /// nested-struct-return budgeting and struct-element
  /// collection scratch sizing.
  pub type_view: Option<(&'a [Ty], &'a TyTable)>,
  /// Concrete element type of struct-element collection
  /// reads/writes, keyed by the `Call`'s destination
  /// `ValueId` (`Sir::vec_elem_tys`).
  pub vec_elem_tys: &'a std::collections::HashMap<u32, TyId>,
}

impl RegAlloc {
  /// Run register allocation on the SIR instruction stream.
  ///
  /// `type_view = Some((tys, ty_table))` enables the deep
  /// struct-return slot accounting that nested-struct
  /// returns rely on: a function returning `Order { qty:
  /// int, shipping: Shipping }` must reserve slots for the
  /// inner Shipping copy too, not just the outer pointer
  /// slot. With `None` the accounting falls back to flat
  /// field count — safe for any program that doesn't
  /// return nested-struct shapes.
  pub fn allocate(input: AllocInput<'_>) -> Self {
    let AllocInput {
      insns,
      next_value_id,
      interner,
      type_view,
      vec_elem_tys,
    } = input;

    let value_ids = compute_value_ids(insns);
    let n = insns.len();
    let mut result = Self {
      assignments: HashMap::default(),
      fp_assignments: HashMap::default(),
      insn_gp: vec![HashMap::default(); n],
      insn_fp: vec![HashMap::default(); n],
      spill_ops: Vec::new(),
      value_ids,
      function_info: HashMap::default(),
      struct_return_fns: HashMap::default(),
      enum_payload_struct_fields: HashMap::default(),
    };

    let functions = find_functions(insns);

    // Clone value_ids to avoid borrow conflict.
    let vids = result.value_ids.clone();

    // Pre-compute the struct-return slot count per
    // user function name in a single linear pass over
    // the whole SIR. Without this, `allocate_function`'s
    // catch-all per-Call branch re-scans the entire
    // instruction stream for every unmatched call —
    // O(calls × insns) overall, which dominated codegen
    // time on programs with many small calls. With
    // `type_view` the count is the *deep* slot count
    // (recursively summed over nested-struct fields) so
    // the call-site deep-copy at codegen has matching
    // stack budget. The map is also exposed on the result
    // (`result.struct_return_fns`) so the codegen's call-
    // site copy loop can reuse it instead of re-scanning
    // the SIR a second time.
    let (struct_return_fns, enum_payload_struct_fields) =
      build_struct_return_map(insns, type_view);

    for (start, end) in functions {
      let ctx = allocator::AllocCtx {
        insns,
        start,
        end,
        value_ids: &vids,
        num_values: next_value_id,
        interner,
        struct_return_fns: &struct_return_fns,
        vec_elem_tys,
        type_view,
      };

      allocator::allocate_function(&ctx, &mut result);
    }

    result.struct_return_fns = struct_return_fns;
    result.enum_payload_struct_fields = enum_payload_struct_fields;

    // Sort `spill_ops` by `insn_idx` so the codegen can
    // index into them in O(1) per insn via a parallel
    // offsets array. `Before` precedes `After` within
    // one insn — the codegen relies on emitting Before
    // first.
    result.spill_ops.sort_by_key(|op| {
      let timing_bit = match op.timing {
        EmitTiming::Before => 0u32,
        EmitTiming::After => 1u32,
      };

      (op.insn_idx as u64) << 1 | timing_bit as u64
    });

    result
  }

  /// Look up the GP register for a ValueId at a specific
  /// instruction index. Falls back to the legacy flat map
  /// when per-instruction data is absent.
  #[inline]
  pub fn get_at(&self, insn_idx: usize, vid: ValueId) -> Option<u8> {
    self
      .insn_gp
      .get(insn_idx)
      .and_then(|m| m.get(&vid.0).copied())
  }

  /// Look up the FP register for a ValueId at a specific
  /// instruction index.
  #[inline]
  pub fn get_fp_at(&self, insn_idx: usize, vid: ValueId) -> Option<u8> {
    self
      .insn_fp
      .get(insn_idx)
      .and_then(|m| m.get(&vid.0).copied())
  }

  /// Flat GP fallback — covers values the per-insn
  /// snapshot misses (liveness/visit_uses gaps).
  #[inline]
  pub fn get(&self, fn_start: u32, vid: ValueId) -> Option<u8> {
    self.assignments.get(&(fn_start, vid.0)).copied()
  }

  /// Flat FP fallback.
  #[inline]
  pub fn get_fp(&self, fn_start: u32, vid: ValueId) -> Option<u8> {
    self.fp_assignments.get(&(fn_start, vid.0)).copied()
  }

  /// Look up the ValueId produced at instruction index.
  #[inline]
  pub fn value_id_at(&self, idx: usize) -> Option<ValueId> {
    self.value_ids.get(idx).copied().flatten()
  }
}

/// Build a map from function name to its struct-return
/// field count, in a single linear pass over the SIR.
/// Only functions whose body emits `StructConstruct` and
/// then `Return Some` are recorded.
///
/// Used by `allocate_function`'s `Insn::Call` budgeting:
/// callers of struct-returning functions reserve space
/// for the struct copy in their own frame. The previous
/// per-call full-SIR scan was O(calls × insns).
fn build_struct_return_map(
  insns: &[Insn],
  type_view: Option<(&[Ty], &TyTable)>,
) -> (HashMap<Symbol, u32>, EnumPayloadFields) {
  let mut map = HashMap::default();
  let mut payload_map: EnumPayloadFields = HashMap::default();
  // Caller → list of callees observed in the caller's
  // body. After the main pass we propagate
  // `payload_map[callee]` into any empty variant slot of
  // `payload_map[caller]` — covers passthrough functions
  // whose body returns the result of a call via a
  // match-arm join slot (no local `EnumConstruct` for
  // that variant). Lookup uses the per-function map only;
  // a TyId-keyed shortcut conflates `Result<X,int>` and
  // `Result<Y,int>` because zo doesn't intern
  // monomorphized enum TyIds.
  let mut callees_of: HashMap<Symbol, Vec<Symbol>> = HashMap::default();
  let mut cur_fn: Option<Symbol> = None;
  let mut cur_fn_return_ty: Option<TyId> = None;
  let mut last_ty: Option<TyId> = None;
  let mut last_fields: Option<u32> = None;

  // Value-id → producing-instruction ty_id, scoped to the
  // currently-walked function. ValueId counters reset
  // across FunDef boundaries, so the map clears on every
  // FunDef — without that scoping, vid=N in one function
  // would alias vid=N in the next and the EnumConstruct
  // lookup below would pick up a stale TyId from a
  // previously-walked function body.
  let mut value_ty: HashMap<u32, TyId> = HashMap::default();

  for insn in insns {
    // Record this insn's `(dst, ty)` so a later
    // `EnumConstruct` whose payload field references `dst`
    // can recover the substituted struct type.
    record_value_ty(insn, &mut value_ty);

    match insn {
      Insn::FunDef {
        name, return_ty, ..
      } => {
        cur_fn = Some(*name);
        cur_fn_return_ty = Some(*return_ty);
        last_ty = None;
        last_fields = None;
        value_ty.clear();

        // Every fn whose declared return is a struct claims
        // a slot, even when the body's tail position is a
        // call (not a `StructConstruct`). FFI composite
        // returns share this path via the AAPCS lift.
        if let Some(slots) = struct_return_slots(*return_ty, type_view) {
          map.insert(*name, slots);
        }
      }
      Insn::StructConstruct { fields, ty_id, .. } => {
        last_fields = Some(fields.len() as u32);
        last_ty = Some(*ty_id);
      }
      Insn::EnumConstruct {
        fields, variant, ..
      } => {
        // Slot budget: 1 (discriminant) + payload fields +
        // each struct-typed payload's recursive cost. The
        // enum's own variant_field_tys are unsubstituted
        // generic placeholders here (zo doesn't intern
        // monomorphized enum TyIds), so we recover the
        // concrete type from each payload value's source
        // insn via the value_ty pre-pass above.
        let mut total = 1 + fields.len() as u32;

        let mut variant_struct_fields: Vec<(u32, TyId)> = Vec::new();

        if let Some((tys, tt)) = type_view {
          for (i, field_vid) in fields.iter().enumerate() {
            if let Some(&fty) = value_ty.get(&field_vid.0)
              && matches!(resolve_ty(tys, fty), Ty::Struct(_))
            {
              total += flat_struct_slots_of(fty, tys, tt).unwrap_or(1);
              variant_struct_fields.push((i as u32, fty));
            }
          }
        }

        // Record the per-variant substituted struct payload
        // fields for the codegen's enum-deep-copy site.
        // Indexed by discriminant; gaps get pre-filled with
        // empty Vecs so a later variant's discriminant slot
        // is in-bounds.
        let disc_idx = *variant as usize;

        if let Some(fname) = cur_fn {
          let entry = payload_map.entry(fname).or_default();

          if entry.len() <= disc_idx {
            entry.resize(disc_idx + 1, Vec::new());
          }

          entry[disc_idx] = variant_struct_fields.clone();
        }

        // Different variants of the same enum SHARE the
        // payload region — only one is alive at a time —
        // so the function's slot budget is the MAX across
        // all variants, not the last one's count.
        last_fields = Some(match last_fields {
          Some(prev) => prev.max(total),
          None => total,
        });
        last_ty = None;
      }
      Insn::Call { name, .. } => {
        if let Some(fname) = cur_fn {
          let entry = callees_of.entry(fname).or_default();

          if !entry.contains(name) {
            entry.push(*name);
          }
        }
      }
      Insn::Return { value: Some(_), .. } => {
        let return_is_composite = cur_fn_return_ty
          .and_then(|rty| struct_return_slots(rty, type_view))
          .is_some();

        if let (Some(fname), Some(n)) = (cur_fn, last_fields)
          && return_is_composite
        {
          // Deep slot count includes the inline copies of
          // any nested-struct fields. With no `type_view`,
          // fall back to the flat field count — preserves
          // the pre-fix budget for any consumer (tests)
          // that hasn't wired a real type table through.
          //
          // When `last_ty` is None (set by the
          // `EnumConstruct` arm above), `n` already
          // includes the enum's 1-slot discriminant +
          // payload + struct deep-copy budget, so we use
          // it directly.
          let deep = match (type_view, last_ty) {
            (Some((tys, tt)), Some(t)) => {
              flat_struct_slots_of(t, tys, tt).unwrap_or(n)
            }
            _ => n,
          };

          // Take the max of the FunDef-derived count
          // (declared return type, e.g. `Result<Self,
          // int>` → 3) and the Return-derived count
          // (last tail `StructConstruct`, e.g. inner
          // `Self {…}` → 1). The latter alone can
          // downgrade the budget; the former alone can
          // miss tail-call shapes.
          map
            .entry(fname)
            .and_modify(|existing| *existing = (*existing).max(deep))
            .or_insert(deep);
        }
      }
      _ => {}
    }
  }

  // Propagate `payload_map` variant entries from each
  // callee into its caller, filling any per-variant slot
  // the caller didn't construct locally. Covers the
  // match-arm passthrough case: `get { match parse_url
  // ... => Result::Fail(e), Result::Pass(_) =>
  // parse_response(text) }` — `get` constructs only the
  // `Fail` variant locally, but `parse_response`'s
  // `Pass` variant carries the `Response` struct payload
  // that `main`'s deep-copy at the `get` call site still
  // needs. Fixpoint covers transitive call chains
  // (`A { B() }; B { C() }`); a well-typed program never
  // dispatches a single match through callees with
  // different substituted payload types in the same
  // variant slot, so a callee's non-empty slot is
  // unambiguous even though `Result<X,int>` and
  // `Result<Y,int>` share the same outer enum `TyId`.
  let mut changed = true;

  while changed {
    changed = false;

    let pairs: Vec<(Symbol, Symbol)> = callees_of
      .iter()
      .flat_map(|(c, cs)| cs.iter().map(move |callee| (*c, *callee)))
      .collect();

    for (caller, callee) in pairs {
      let Some(callee_entry) = payload_map.get(&callee).cloned() else {
        continue;
      };

      let caller_entry = payload_map.entry(caller).or_default();

      if caller_entry.len() < callee_entry.len() {
        caller_entry.resize(callee_entry.len(), Vec::new());
      }

      for (i, callee_v) in callee_entry.iter().enumerate() {
        if !callee_v.is_empty() && caller_entry[i].is_empty() {
          caller_entry[i] = callee_v.clone();
          changed = true;
        }
      }
    }
  }

  (map, payload_map)
}

/// Record `insn.dst → insn.ty` in `value_ty` when the
/// insn is a value-producing one. Used by
/// [`build_struct_return_map`]'s `EnumConstruct` arm to
/// recover the substituted type of each payload field
/// from its source ValueId.
fn record_value_ty(insn: &Insn, value_ty: &mut HashMap<u32, TyId>) {
  use Insn::*;
  match insn {
    ConstInt { dst, ty_id, .. }
    | ConstFloat { dst, ty_id, .. }
    | ConstBool { dst, ty_id, .. }
    | ConstString { dst, ty_id, .. }
    | Load { dst, ty_id, .. }
    | Call { dst, ty_id, .. }
    | BinOp { dst, ty_id, .. }
    | UnOp { dst, ty_id, .. }
    | StructConstruct { dst, ty_id, .. }
    | EnumConstruct { dst, ty_id, .. }
    | TupleLiteral { dst, ty_id, .. }
    | TupleIndex { dst, ty_id, .. }
    | ArrayLiteral { dst, ty_id, .. }
    | ArrayIndex { dst, ty_id, .. } => {
      value_ty.insert(dst.0, *ty_id);
    }
    Cast { dst, to_ty, .. } => {
      value_ty.insert(dst.0, *to_ty);
    }
    _ => {}
  }
}

/// Slots required to deep-copy a value of type `ty_id`
/// when it crosses a struct-return boundary. Primitives
/// (or any non-struct) collapse to a single 8-byte slot
/// — the AArch64 word that carries either the value or
/// a pointer. Structs reserve one slot per top-level
/// field PLUS the recursive cost of every nested-struct
/// field, so the caller-side copy can root the inner
/// payload inside its own frame instead of trailing the
/// pointer back into the (freed) callee frame.
///
/// Returns `None` if the type table lookup fails (e.g.
/// an interned struct that didn't make it across module
/// boundaries) — caller decides the fallback. In
/// practice the regalloc and codegen both fall back to
/// the SIR-observed flat field count, which keeps the
/// program building even if it can't deep-copy.
pub fn flat_struct_slots_of(
  ty_id: TyId,
  tys: &[Ty],
  ty_table: &TyTable,
) -> Option<u32> {
  match resolve_ty(tys, ty_id) {
    Ty::Struct(sid) => {
      let st = ty_table.struct_ty(sid)?;
      let fields = ty_table.struct_fields(st);

      let mut total = fields.len() as u32;

      for field in fields {
        if matches!(resolve_ty(tys, field.ty_id), Ty::Struct(_)) {
          total += flat_struct_slots_of(field.ty_id, tys, ty_table)?;
        }
      }

      Some(total)
    }
    Ty::Enum(eid) => {
      // Layout for an enum returned across the call
      // boundary:
      //
      //   [ disc ] [ payload₀ ] [ payload₁ ] ...
      //
      // The first `outer = 1 + max(variant.field_count)`
      // slots hold the discriminant and per-variant
      // payload values (or pointers to structs on the
      // callee frame).
      //
      // For every variant whose payload contains struct
      // fields, the caller ALSO needs scratch slots to
      // deep-copy the struct out of the callee frame
      // (which is gone after RET). We take the worst-
      // case over all variants — variants share the
      // same payload region, so only one variant's
      // nested struct fields ever materialize at a
      // time.
      //
      // The nested-struct slot budget is what gives the
      // caller-side deep-copy a place to land each
      // variant's struct payload; without it the
      // payload pointer dangles into the (gone) callee
      // frame.
      let e = ty_table.enum_ty(eid)?;
      let variants = ty_table.enum_variants(e);

      let max_payload =
        variants.iter().map(|v| v.field_count).max().unwrap_or(0);
      let outer = 1 + max_payload;

      let mut max_nested = 0u32;

      for v in variants {
        let fields = ty_table.variant_fields(v);
        let mut nested = 0u32;

        for &field_ty in fields {
          if matches!(resolve_ty(tys, field_ty), Ty::Struct(_)) {
            nested +=
              flat_struct_slots_of(field_ty, tys, ty_table).unwrap_or(1);
          }
        }

        max_nested = max_nested.max(nested);
      }

      Some(outer + max_nested)
    }
    _ => Some(1),
  }
}

/// Slots a struct-returning fn claims in its caller's frame.
///
/// @note — `None` when `return_ty` isn't a struct, or when
/// the type-view is absent / the struct id is missing.
/// Drives `build_struct_return_map`'s registration filter.
fn struct_return_slots(
  return_ty: TyId,
  type_view: Option<(&[Ty], &TyTable)>,
) -> Option<u32> {
  let (tys, tt) = type_view?;

  match resolve_ty(tys, return_ty) {
    Ty::Struct(_) | Ty::Enum(_) => flat_struct_slots_of(return_ty, tys, tt),
    _ => None,
  }
}

/// Look up `Ty` for `id` in the `tys` slice, falling back
/// to `Ty::Error` when the index is out of bounds. The
/// fallback keeps callers crash-free against type tables
/// that haven't seen the interned id yet (cross-module
/// translation edges, error recovery).
#[inline]
pub fn resolve_ty(tys: &[Ty], id: TyId) -> Ty {
  tys.get(id.0 as usize).copied().unwrap_or(Ty::Error)
}

/// Identify non-intrinsic function bodies as (start, end)
/// ranges into the instruction stream.
fn find_functions(insns: &[Insn]) -> Vec<(usize, usize)> {
  // Collect all FunDef positions.
  let positions = insns
    .iter()
    .enumerate()
    .filter_map(|(i, insn)| match insn {
      Insn::FunDef { kind, .. } => Some((i, *kind)),
      _ => None,
    })
    .collect::<Vec<_>>();

  let mut result = Vec::new();

  for (j, &(start, kind)) in positions.iter().enumerate() {
    if kind == FunctionKind::Intrinsic {
      continue;
    }

    let end = if j + 1 < positions.len() {
      positions[j + 1].0
    } else {
      insns.len()
    };

    result.push((start, end));
  }

  result
}
