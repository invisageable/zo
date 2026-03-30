use crate::{
  ALLOCATABLE_FP, ALLOCATABLE_GP, EmitTiming, FunctionInfo, RegAlloc,
  RegisterClass, SpillKind, SpillOp,
};
use zo_liveness::{LivenessInfo, liveness};
use zo_sir::{Insn, LoadSource};
use zo_value::FunctionKind;
use zo_value::ValueId;

use rustc_hash::FxHashMap;

/// A single register pool (GP or FP).
struct RegPool {
  free: Vec<u8>,
  val_to_reg: FxHashMap<u32, u8>,
  reg_to_val: FxHashMap<u8, u32>,
}

impl RegPool {
  fn new(regs: &[u8]) -> Self {
    Self {
      free: regs.iter().rev().copied().collect(),
      val_to_reg: FxHashMap::default(),
      reg_to_val: FxHashMap::default(),
    }
  }

  fn get(&self, vid: ValueId) -> Option<u8> {
    self.val_to_reg.get(&vid.0).copied()
  }

  fn assign(&mut self, vid: ValueId, reg: u8) {
    self.val_to_reg.insert(vid.0, reg);
    self.reg_to_val.insert(reg, vid.0);
    self.free.retain(|&r| r != reg);
  }

  fn free_value(&mut self, vid: ValueId) {
    if let Some(reg) = self.val_to_reg.remove(&vid.0) {
      self.reg_to_val.remove(&reg);
      self.free.push(reg);
    }
  }

  fn alloc_free(&mut self) -> Option<u8> {
    self.free.pop()
  }

  fn clear(&mut self, regs: &[u8]) {
    self.val_to_reg.clear();
    self.reg_to_val.clear();
    self.free = regs.iter().rev().copied().collect();
  }
}

/// Mutable state for the forward allocation pass.
struct AllocState {
  gp: RegPool,
  fp: RegPool,
  /// Tracks which values are FP (for correct spill emission).
  is_fp_value: FxHashMap<u32, bool>,
  /// Spill slots: ValueId.0 → slot index.
  spill_slots: FxHashMap<u32, u32>,
  /// Next spill slot index.
  next_spill: u32,
}

impl AllocState {
  fn new() -> Self {
    Self {
      gp: RegPool::new(&ALLOCATABLE_GP),
      fp: RegPool::new(&ALLOCATABLE_FP),
      is_fp_value: FxHashMap::default(),
      spill_slots: FxHashMap::default(),
      next_spill: 0,
    }
  }

  /// Look up the register holding a value (GP or FP).
  fn get(&self, vid: ValueId) -> Option<u8> {
    self.gp.get(vid).or_else(|| self.fp.get(vid))
  }

  fn is_fp(&self, vid: ValueId) -> bool {
    self.is_fp_value.get(&vid.0).copied().unwrap_or(false)
  }

  fn pool_mut(&mut self, fp: bool) -> &mut RegPool {
    if fp { &mut self.fp } else { &mut self.gp }
  }

  fn assign(&mut self, vid: ValueId, reg: u8, fp: bool) {
    self.is_fp_value.insert(vid.0, fp);
    self.pool_mut(fp).assign(vid, reg);
  }

  fn free_value(&mut self, vid: ValueId) {
    if self.is_fp(vid) {
      self.fp.free_value(vid);
    } else {
      self.gp.free_value(vid);
    }
    self.is_fp_value.remove(&vid.0);
  }

  fn spill_slot(&mut self, vid: u32) -> u32 {
    if let Some(&slot) = self.spill_slots.get(&vid) {
      return slot;
    }
    let slot = self.next_spill;
    self.next_spill += 1;
    self.spill_slots.insert(vid, slot);
    slot
  }

  fn evict(
    &mut self,
    vid: u32,
    insn_idx: usize,
    liveness: &LivenessInfo,
    local_idx: usize,
    result: &mut RegAlloc,
  ) -> u8 {
    let fp = self.is_fp(ValueId(vid));
    let reg = self.pool_mut(fp).val_to_reg[&vid];

    if liveness.live_out[local_idx].test(vid as usize) {
      let slot = self.spill_slot(vid);

      result.spill_ops.push(SpillOp {
        insn_idx,
        timing: EmitTiming::Before,
        kind: SpillKind::Store {
          reg,
          slot,
          class: if fp {
            RegisterClass::FP
          } else {
            RegisterClass::GP
          },
        },
      });
    }

    let pool = self.pool_mut(fp);

    pool.val_to_reg.remove(&vid);
    pool.reg_to_val.remove(&reg);
    self.is_fp_value.remove(&vid);

    reg
  }

  fn alloc_or_spill(
    &mut self,
    insn_idx: usize,
    liveness: &LivenessInfo,
    local_idx: usize,
    result: &mut RegAlloc,
    fp: bool,
  ) -> u8 {
    let pool = self.pool_mut(fp);

    if let Some(reg) = pool.alloc_free() {
      return reg;
    }

    // Spill from the correct pool.
    let victim = *pool
      .val_to_reg
      .keys()
      .next()
      .expect("register pool exhausted");

    self.evict(victim, insn_idx, liveness, local_idx, result)
  }

  fn clear_all(&mut self) {
    self.gp.clear(&ALLOCATABLE_GP);
    self.fp.clear(&ALLOCATABLE_FP);
    self.is_fp_value.clear();
  }
}

/// Run the forward allocation pass for a single function.
///
/// `insns[start..end]` is the function body (FunDef to next
/// FunDef / end). Results are merged into `result`.
pub fn allocate_function(
  insns: &[Insn],
  start: usize,
  end: usize,
  value_ids: &[Option<ValueId>],
  num_values: u32,
  result: &mut RegAlloc,
) {
  let n = end - start;

  if n == 0 {
    return;
  }

  // Extract params.
  let (params, fn_kind) = match &insns[start] {
    Insn::FunDef { params, kind, .. } => (params.clone(), *kind),
    _ => return,
  };

  if fn_kind == FunctionKind::Intrinsic {
    return;
  }

  // Liveness analysis.
  let liveness = liveness::analyze(insns, start, end, value_ids, num_values);

  let mut state = AllocState::new();
  let mut has_calls = false;

  // Reserve parameter registers in the correct pool.
  let n_params = params.len().min(8);

  for (i, (_name, ty_id)) in params.iter().enumerate().take(n_params) {
    let is_fp = ty_id.0 >= 15 && ty_id.0 <= 17;

    if is_fp {
      state.fp.free.retain(|&r| r != i as u8);
    } else {
      state.gp.free.retain(|&r| r != i as u8);
    }
  }

  for i in 0..n {
    let gi = start + i;
    let insn = &insns[gi];
    let fp = insn_is_fp(insn);

    // --- Handle Load (parameter or mutable) ---
    if let Insn::Load { dst, src, .. } = insn {
      match src {
        LoadSource::Param(_) => {
          // Allocate a fresh register — don't force the
          // physical param register. The codegen will load
          // from the param's spill slot on the stack.
          let reg = state.alloc_or_spill(gi, &liveness, i, result, fp);

          state.assign(*dst, reg, fp);
          insert_assignment(result, *dst, reg, fp);
        }
        LoadSource::Local(_) => {
          // Local variable: allocate a fresh register
          // (the value will be loaded from stack at
          // runtime by the codegen LDR).
          let reg = state.alloc_or_spill(gi, &liveness, i, result, fp);

          state.assign(*dst, reg, fp);
          insert_assignment(result, *dst, reg, fp);
        }
      }

      free_dead(&mut state, &liveness, i);

      continue;
    }

    // --- Handle Call (clobbers all caller-saved) ---
    if let Insn::Call { args, .. } = insn {
      has_calls = true;

      let arg_set = args.iter().map(|a| a.0).collect::<Vec<_>>();

      // Collect values to save (both GP and FP).
      let gp_save = state
        .gp
        .val_to_reg
        .iter()
        .filter(|(vid, _)| {
          liveness.live_out[i].test(**vid as usize) && !arg_set.contains(vid)
        })
        .map(|(vid, reg)| (*vid, *reg))
        .collect::<Vec<_>>();

      let fp_save = state
        .fp
        .val_to_reg
        .iter()
        .filter(|(vid, _)| {
          liveness.live_out[i].test(**vid as usize) && !arg_set.contains(vid)
        })
        .map(|(vid, reg)| (*vid, *reg))
        .collect::<Vec<_>>();

      for &(vid, reg) in &gp_save {
        let slot = state.spill_slot(vid);

        result.spill_ops.push(SpillOp {
          insn_idx: gi,
          timing: EmitTiming::Before,
          kind: SpillKind::Store {
            reg,
            slot,
            class: RegisterClass::GP,
          },
        });
      }

      for &(vid, reg) in &fp_save {
        let slot = state.spill_slot(vid);

        result.spill_ops.push(SpillOp {
          insn_idx: gi,
          timing: EmitTiming::Before,
          kind: SpillKind::Store {
            reg,
            slot,
            class: RegisterClass::FP,
          },
        });
      }

      state.clear_all();

      // Call result goes to X0 (GP) or D0 (FP).
      let result_fp = insn_is_fp(insn);

      if let Some(vid) = value_ids[gi] {
        let reg = 0; // X0 or D0

        state.assign(vid, reg, result_fp);
        insert_assignment(result, vid, reg, result_fp);
      }

      // Reload saved values.
      if gi + 1 < end {
        for &(vid, _) in &gp_save {
          let slot = state.spill_slots[&vid];
          let reg = state.gp.alloc_free().expect("out of GP regs for reload");

          result.spill_ops.push(SpillOp {
            insn_idx: gi + 1,
            timing: EmitTiming::Before,
            kind: SpillKind::Load {
              reg,
              slot,
              class: RegisterClass::GP,
            },
          });

          state.assign(ValueId(vid), reg, false);
          result.assignments.insert(vid, reg);
        }
        for &(vid, _) in &fp_save {
          let slot = state.spill_slots[&vid];
          let reg = state.fp.alloc_free().expect("out of FP regs for reload");

          result.spill_ops.push(SpillOp {
            insn_idx: gi + 1,
            timing: EmitTiming::Before,
            kind: SpillKind::Load {
              reg,
              slot,
              class: RegisterClass::FP,
            },
          });

          state.assign(ValueId(vid), reg, true);
          result.fp_assignments.insert(vid, reg);
        }
      }

      free_dead(&mut state, &liveness, i);

      continue;
    }

    // --- General case ---

    let uses = zo_liveness::insn_uses(insn);

    for &use_vid in &uses {
      if use_vid.0 == u32::MAX {
        continue;
      }

      if state.get(use_vid).is_some() {
        continue;
      }

      if let Some(&slot) = state.spill_slots.get(&use_vid.0) {
        let ufp = state.is_fp(use_vid);
        let reg = state.alloc_or_spill(gi, &liveness, i, result, ufp);

        result.spill_ops.push(SpillOp {
          insn_idx: gi,
          timing: EmitTiming::Before,
          kind: SpillKind::Load {
            reg,
            slot,
            class: if ufp {
              RegisterClass::FP
            } else {
              RegisterClass::GP
            },
          },
        });

        state.assign(use_vid, reg, ufp);
        insert_assignment(result, use_vid, reg, ufp);
      }
    }

    for &use_vid in &uses {
      if use_vid.0 == u32::MAX {
        continue;
      }

      if !liveness.live_out[i].test(use_vid.0 as usize) {
        state.free_value(use_vid);
      }
    }

    if let Some(vid) = value_ids[gi] {
      let reg = state.alloc_or_spill(gi, &liveness, i, result, fp);

      state.assign(vid, reg, fp);
      insert_assignment(result, vid, reg, fp);
    }

    free_dead(&mut state, &liveness, i);
  }

  // Record per-function info.
  let spill_count = state.next_spill;
  let spill_size = (spill_count * 8 + 15) & !15;

  // Compute total struct/enum allocation space needed.
  let mut struct_slots: u32 = 0;

  for insn in &insns[start..end] {
    match insn {
      Insn::StructConstruct { fields, .. } => {
        struct_slots += fields.len() as u32;
      }
      Insn::EnumConstruct { fields, .. } if !fields.is_empty() => {
        // tag + fields.
        struct_slots += 1 + fields.len() as u32;
      }
      Insn::ArrayLiteral { elements, .. } => {
        // length header + elements.
        struct_slots += 1 + elements.len() as u32;
      }
      Insn::TupleLiteral { elements, .. } => {
        struct_slots += elements.len() as u32;
      }
      _ => {}
    }
  }

  let struct_size = (struct_slots * 8 + 15) & !15;

  result.function_info.insert(
    start,
    FunctionInfo {
      has_calls,
      spill_count,
      spill_size,
      struct_size,
    },
  );
}

/// Free all active values that are NOT in live_out.
fn free_dead(
  state: &mut AllocState,
  liveness: &LivenessInfo,
  local_idx: usize,
) {
  let dead = state
    .gp
    .val_to_reg
    .keys()
    .chain(state.fp.val_to_reg.keys())
    .filter(|vid| !liveness.live_out[local_idx].test(**vid as usize))
    .copied()
    .collect::<Vec<_>>();

  for vid in dead {
    state.free_value(ValueId(vid));
  }
}

/// Determine if a SIR instruction produces a float value.
fn insn_is_fp(insn: &Insn) -> bool {
  match insn {
    Insn::ConstFloat { .. } => true,
    Insn::BinOp { ty_id, .. }
    | Insn::UnOp { ty_id, .. }
    | Insn::Load { ty_id, .. }
    | Insn::Call { ty_id, .. } => ty_id.0 >= 15 && ty_id.0 <= 17,
    _ => false,
  }
}

/// Insert assignment into the correct map (GP or FP).
fn insert_assignment(result: &mut RegAlloc, vid: ValueId, reg: u8, fp: bool) {
  if fp {
    result.fp_assignments.insert(vid.0, reg);
  } else {
    result.assignments.insert(vid.0, reg);
  }
}
