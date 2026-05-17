pub mod allocator;

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
  /// `_zo_chan_send(chan, src)` reads it through its
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
  },
}

/// The result of register allocation over the entire SIR.
pub struct RegAlloc {
  /// `(function_start, ValueId.0) → GP register`. Keyed
  /// per-function because ValueId counters reset across
  /// FunDefs; a flat `vid → reg` map would let a later
  /// function's vid=N overwrite an earlier one's.
  pub assignments: HashMap<(u32, u32), u8>,
  /// `(function_start, ValueId.0) → FP register`. Same
  /// scoping as [`Self::assignments`].
  pub fp_assignments: HashMap<(u32, u32), u8>,
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
  pub fn allocate(
    insns: &[Insn],
    next_value_id: u32,
    interner: &Interner,
    type_view: Option<(&[Ty], &TyTable)>,
  ) -> Self {
    let value_ids = compute_value_ids(insns);
    let mut result = Self {
      assignments: HashMap::default(),
      fp_assignments: HashMap::default(),
      spill_ops: Vec::new(),
      value_ids,
      function_info: HashMap::default(),
      struct_return_fns: HashMap::default(),
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
    let struct_return_fns = build_struct_return_map(insns, type_view);

    for (start, end) in functions {
      let ctx = allocator::AllocCtx {
        insns,
        start,
        end,
        value_ids: &vids,
        num_values: next_value_id,
        interner,
        struct_return_fns: &struct_return_fns,
      };

      allocator::allocate_function(&ctx, &mut result);
    }

    result.struct_return_fns = struct_return_fns;

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

  /// Look up the GP register for a ValueId within the
  /// function whose body starts at `fn_start`.
  #[inline]
  pub fn get(&self, fn_start: u32, vid: ValueId) -> Option<u8> {
    self.assignments.get(&(fn_start, vid.0)).copied()
  }

  /// Look up the FP register for a ValueId within the
  /// function whose body starts at `fn_start`.
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
) -> HashMap<Symbol, u32> {
  let mut map = HashMap::default();
  let mut cur_fn: Option<Symbol> = None;
  let mut last_ty: Option<TyId> = None;
  let mut last_fields: Option<u32> = None;

  for insn in insns {
    match insn {
      Insn::FunDef { name, .. } => {
        cur_fn = Some(*name);
        last_ty = None;
        last_fields = None;
      }
      Insn::StructConstruct { fields, ty_id, .. } => {
        last_fields = Some(fields.len() as u32);
        last_ty = Some(*ty_id);
      }
      Insn::Return { value: Some(_), .. } => {
        if let (Some(fname), Some(n)) = (cur_fn, last_fields) {
          // Deep slot count includes the inline copies of
          // any nested-struct fields. With no `type_view`,
          // fall back to the flat field count — preserves
          // the pre-fix budget for any consumer (tests)
          // that hasn't wired a real type table through.
          let deep = match (type_view, last_ty) {
            (Some((tys, tt)), Some(t)) => {
              flat_struct_slots_of(t, tys, tt).unwrap_or(n)
            }
            _ => n,
          };

          map.insert(fname, deep);
        }
      }
      _ => {}
    }
  }

  map
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
    _ => Some(1),
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
