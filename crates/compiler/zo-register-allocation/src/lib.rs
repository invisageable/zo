pub mod allocator;

use zo_sir::Insn;
use zo_value::{FunctionKind, ValueId};

use rustc_hash::FxHashMap as HashMap;

// Re-export liveness utilities so existing consumers
// don't need to add zo-liveness directly.
pub use zo_liveness::{compute_value_ids, insn_uses};

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
  /// ValueId.0 → GP register index.
  pub assignments: HashMap<u32, u8>,
  /// ValueId.0 → FP register index.
  pub fp_assignments: HashMap<u32, u8>,
  /// Spill operations emitted by the allocator.
  pub spill_ops: Vec<SpillOp>,
  /// ValueId produced by each instruction (parallel array).
  pub value_ids: Vec<Option<ValueId>>,
  /// Per-function info, keyed by function start index.
  pub function_info: HashMap<usize, FunctionInfo>,
}

impl RegAlloc {
  /// Run register allocation on the SIR instruction stream.
  pub fn allocate(
    insns: &[Insn],
    next_value_id: u32,
    interner: &zo_interner::Interner,
  ) -> Self {
    let value_ids = compute_value_ids(insns);
    let mut result = Self {
      assignments: HashMap::default(),
      fp_assignments: HashMap::default(),
      spill_ops: Vec::new(),
      value_ids,
      function_info: HashMap::default(),
    };

    let functions = find_functions(insns);

    // Clone value_ids to avoid borrow conflict.
    let vids = result.value_ids.clone();

    for (start, end) in functions {
      allocator::allocate_function(
        insns,
        start,
        end,
        &vids,
        next_value_id,
        &mut result,
        interner,
      );
    }

    result
  }

  /// Look up the GP register for a ValueId.
  #[inline]
  pub fn get(&self, vid: ValueId) -> Option<u8> {
    self.assignments.get(&vid.0).copied()
  }

  /// Look up the FP register for a ValueId.
  #[inline]
  pub fn get_fp(&self, vid: ValueId) -> Option<u8> {
    self.fp_assignments.get(&vid.0).copied()
  }

  /// Look up the ValueId produced at instruction index.
  #[inline]
  pub fn value_id_at(&self, idx: usize) -> Option<ValueId> {
    self.value_ids.get(idx).copied().flatten()
  }
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
