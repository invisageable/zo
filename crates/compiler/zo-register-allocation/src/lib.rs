pub mod allocator;
pub mod liveness;

use zo_sir::Insn;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

/// Caller-saved GP register indices, preferred order.
/// Temps (X9-X15) first, then args (X0-X7).
pub const ALLOCATABLE_GP: [u8; 15] =
  [9, 10, 11, 12, 13, 14, 15, 0, 1, 2, 3, 4, 5, 6, 7];

/// Caller-saved FP register indices, preferred order.
/// Temps (D16-D31) first, then args (D0-D7).
pub const ALLOCATABLE_FP: [u8; 24] = [
  16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 0, 1, 2, 3,
  4, 5, 6, 7,
];

/// Per-function metadata produced by the allocator.
pub struct FunctionInfo {
  /// Whether the function contains any Call instructions.
  pub has_calls: bool,
  /// Number of spill slots used.
  pub spill_count: u32,
  /// Stack space for spills, aligned to 16.
  pub spill_size: u32,
}

/// A spill operation to emit during codegen.
pub struct SpillOp {
  /// SIR instruction index this is associated with.
  pub insn_idx: usize,
  /// Emit before (true) or after (false) the instruction.
  pub before: bool,
  /// The load or store to emit.
  pub kind: SpillKind,
}

/// Spill operation kind.
#[derive(Clone)]
pub enum SpillKind {
  /// STR reg, [SP, #slot*8]
  Store { reg: u8, slot: u32, is_fp: bool },
  /// LDR reg, [SP, #slot*8]
  Load { reg: u8, slot: u32, is_fp: bool },
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
  pub fn allocate(insns: &[Insn], next_value_id: u32) -> Self {
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

/// Compute the ValueId produced by each SIR instruction.
///
/// Replays the numbering logic from `Sir::emit()`:
/// - Load / BinOp have explicit `dst`.
/// - FunDef, Return, VarDef, Store, ModuleLoad, PackDecl,
///   Label, Jump, BranchIfNot produce no value.
/// - Everything else auto-increments a counter.
pub fn compute_value_ids(insns: &[Insn]) -> Vec<Option<ValueId>> {
  let mut counter = 0u32;

  insns
    .iter()
    .map(|insn| match insn {
      Insn::Load { dst, .. }
      | Insn::BinOp { dst, .. }
      | Insn::ArrayIndex { dst, .. }
      | Insn::ArrayLen { dst, .. } => {
        counter = counter.max(dst.0 + 1);
        Some(*dst)
      }
      Insn::FunDef { .. }
      | Insn::Return { .. }
      | Insn::VarDef { .. }
      | Insn::Store { .. }
      | Insn::ModuleLoad { .. }
      | Insn::PackDecl { .. }
      | Insn::Label { .. }
      | Insn::Jump { .. }
      | Insn::BranchIfNot { .. } => None,
      _ => {
        let id = ValueId(counter);
        counter += 1;
        Some(id)
      }
    })
    .collect()
}

/// Extract the ValueIds read by an instruction.
pub fn insn_uses(insn: &Insn) -> Vec<ValueId> {
  match insn {
    Insn::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
    Insn::Return { value: Some(v), .. } => vec![*v],
    Insn::Store { value, .. } => vec![*value],
    Insn::Call { args, .. } => args.clone(),
    Insn::UnOp { rhs, .. } => vec![*rhs],
    Insn::BranchIfNot { cond, .. } => vec![*cond],
    Insn::Directive { value, .. } => vec![*value],
    Insn::VarDef { init: Some(v), .. } => vec![*v],
    Insn::ArrayLiteral { elements, .. } => elements.clone(),
    Insn::ArrayIndex { array, index, .. } => {
      vec![*array, *index]
    }
    Insn::ArrayLen { array, .. } => vec![*array],
    _ => vec![],
  }
}

/// Identify non-intrinsic function bodies as (start, end)
/// ranges into the instruction stream.
fn find_functions(insns: &[Insn]) -> Vec<(usize, usize)> {
  // Collect all FunDef positions.
  let positions: Vec<(usize, bool)> = insns
    .iter()
    .enumerate()
    .filter_map(|(i, insn)| match insn {
      Insn::FunDef { is_intrinsic, .. } => Some((i, *is_intrinsic)),
      _ => None,
    })
    .collect();

  let mut result = Vec::new();

  for (j, &(start, is_intrinsic)) in positions.iter().enumerate() {
    if is_intrinsic {
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
