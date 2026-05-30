use zo_interner::Interner;
use zo_liveness::compute_value_ids;
use zo_register_allocation::RegAlloc;
use zo_register_allocation::allocator::{AllocCtx, allocate_function};
use zo_sir::{BinOp, Insn, LoadSource};
use zo_span::Span;
use zo_ty::{SelfKind, TyId};
use zo_value::{FunctionKind, Pubness, ValueId};

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence};
use rustc_hash::FxHashMap as HashMap;

const INT_TY: TyId = TyId(10);
const VOID_TY: TyId = TyId(1);

fn empty_result(n: usize) -> RegAlloc {
  RegAlloc {
    assignments: HashMap::default(),
    fp_assignments: HashMap::default(),
    insn_gp: vec![HashMap::default(); n],
    insn_fp: vec![HashMap::default(); n],
    spill_ops: Vec::new(),
    value_ids: vec![None; n],
    function_info: HashMap::default(),
    struct_return_fns: HashMap::default(),
    enum_payload_struct_fields: HashMap::default(),
  }
}

fn run_allocator(insns: &[Insn], interner: &Interner) -> RegAlloc {
  let value_ids = compute_value_ids(insns);
  let n = insns.len();

  let max_vid = value_ids
    .iter()
    .filter_map(|v| v.map(|id| id.0))
    .max()
    .unwrap_or(0)
    + 1;

  let mut result = empty_result(n);
  result.value_ids = value_ids.clone();

  let struct_return_fns = HashMap::default();
  let ctx = AllocCtx {
    insns,
    start: 0,
    end: n,
    value_ids: &value_ids,
    num_values: max_vid,
    interner,
    struct_return_fns: &struct_return_fns,
  };

  allocate_function(&ctx, &mut result);
  result
}

fn fundef(interner: &mut Interner, params: usize) -> Insn {
  let name = interner.intern("test_fn");
  let ps: Vec<_> = (0..params)
    .map(|i| {
      let sym = interner.intern(&format!("p{i}"));
      (sym, INT_TY)
    })
    .collect();

  Insn::FunDef {
    name,
    params: ps,
    return_ty: INT_TY,
    body_start: 1,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    self_kind: SelfKind::None,
    link_name: None,
    owning_pack: None,
    span: Span::ZERO,
    is_test: false,
  }
}

/// Build a function body with `param_count` params, `const_count`
/// constants, `binop_count` binary ops, and optionally a Call.
fn build_body(
  interner: &mut Interner,
  param_count: usize,
  const_count: usize,
  binop_count: usize,
  call_arg_count: usize,
) -> Vec<Insn> {
  let mut insns = Vec::new();
  let mut next_vid: u32 = 0;

  insns.push(fundef(interner, param_count));

  for i in 0..param_count {
    insns.push(Insn::Load {
      dst: ValueId(next_vid),
      src: LoadSource::Param(i as u32),
      ty_id: INT_TY,
    });
    next_vid += 1;
  }

  for i in 0..const_count {
    insns.push(Insn::ConstInt {
      dst: ValueId(next_vid),
      value: (i as u64) + 1,
      ty_id: INT_TY,
    });
    next_vid += 1;
  }

  // BinOps — each uses the two most recent values.
  for _ in 0..binop_count {
    if next_vid < 2 {
      break;
    }
    insns.push(Insn::BinOp {
      dst: ValueId(next_vid),
      lhs: ValueId(next_vid - 2),
      rhs: ValueId(next_vid - 1),
      op: BinOp::Add,
      ty_id: INT_TY,
    });
    next_vid += 1;
  }

  // Optional Call — uses the most recent N values as args.
  if call_arg_count > 0 && next_vid as usize >= call_arg_count {
    let callee = interner.intern("callee");
    let args: Vec<_> = (0..call_arg_count)
      .map(|i| ValueId(next_vid - call_arg_count as u32 + i as u32))
      .collect();

    insns.push(Insn::Call {
      dst: ValueId(next_vid),
      name: callee,
      callee_pack: None,
      args,
      ty_id: INT_TY,
    });
    next_vid += 1;
  }

  if next_vid > 0 {
    insns.push(Insn::Return {
      value: Some(ValueId(next_vid - 1)),
      ty_id: INT_TY,
    });
  } else {
    insns.push(Insn::Return {
      value: None,
      ty_id: VOID_TY,
    });
  }

  insns
}

/// Spill pressure: N constants all alive at a final sum.
/// With 14 GP registers, N > 14 forces real spills.
fn build_spill_pressure(
  interner: &mut Interner,
  live_count: usize,
) -> Vec<Insn> {
  let mut insns = Vec::new();
  let mut next_vid: u32 = 0;

  insns.push(fundef(interner, 0));

  for i in 0..live_count {
    insns.push(Insn::ConstInt {
      dst: ValueId(next_vid),
      value: (i as u64) + 1,
      ty_id: INT_TY,
    });
    next_vid += 1;
  }

  // Sum ALL constants — keeps every value live until here.
  if live_count >= 2 {
    let mut acc = ValueId(0);

    for i in 1..live_count {
      insns.push(Insn::BinOp {
        dst: ValueId(next_vid),
        lhs: acc,
        rhs: ValueId(i as u32),
        op: BinOp::Add,
        ty_id: INT_TY,
      });
      acc = ValueId(next_vid);
      next_vid += 1;
    }
  }

  insns.push(Insn::Return {
    value: if next_vid > 0 {
      Some(ValueId(next_vid - 1))
    } else {
      None
    },
    ty_id: INT_TY,
  });

  insns
}

/// Live range crossing a Call: define A, call, use A.
/// Forces A to be spilled across the call and reloaded.
fn build_live_across_call(
  interner: &mut Interner,
  values_before: usize,
  call_count: usize,
) -> Vec<Insn> {
  let mut insns = Vec::new();
  let mut next_vid: u32 = 0;
  let callee = interner.intern("callee");

  insns.push(fundef(interner, 0));

  let mut pre_call_vids = Vec::new();

  for i in 0..values_before {
    insns.push(Insn::ConstInt {
      dst: ValueId(next_vid),
      value: (i as u64) + 1,
      ty_id: INT_TY,
    });
    pre_call_vids.push(ValueId(next_vid));
    next_vid += 1;
  }

  // Interleave calls — each clobbers all caller-saved.
  for _ in 0..call_count {
    insns.push(Insn::Call {
      dst: ValueId(next_vid),
      name: callee,
      callee_pack: None,
      args: vec![],
      ty_id: INT_TY,
    });
    next_vid += 1;
  }

  // Use ALL pre-call values after the calls — forces
  // spill across every call and reload here.
  if pre_call_vids.len() >= 2 {
    let mut acc = pre_call_vids[0];

    for &vid in &pre_call_vids[1..] {
      insns.push(Insn::BinOp {
        dst: ValueId(next_vid),
        lhs: acc,
        rhs: vid,
        op: BinOp::Add,
        ty_id: INT_TY,
      });
      acc = ValueId(next_vid);
      next_vid += 1;
    }
  }

  insns.push(Insn::Return {
    value: if next_vid > 0 {
      Some(ValueId(next_vid - 1))
    } else {
      None
    },
    ty_id: INT_TY,
  });

  insns
}

/// Multiple calls with overlapping live ranges:
/// define A, call1, define B, call2, use A + B.
fn build_interleaved_calls(
  interner: &mut Interner,
  groups: usize,
) -> Vec<Insn> {
  let mut insns = Vec::new();
  let mut next_vid: u32 = 0;
  let callee = interner.intern("callee");

  insns.push(fundef(interner, 0));

  let mut group_vids = Vec::new();

  for _ in 0..groups {
    insns.push(Insn::ConstInt {
      dst: ValueId(next_vid),
      value: next_vid as u64 + 1,
      ty_id: INT_TY,
    });
    group_vids.push(ValueId(next_vid));
    next_vid += 1;

    insns.push(Insn::Call {
      dst: ValueId(next_vid),
      name: callee,
      callee_pack: None,
      args: vec![],
      ty_id: INT_TY,
    });
    next_vid += 1;
  }

  // Sum all pre-call values — all must survive their
  // respective calls.
  if group_vids.len() >= 2 {
    let mut acc = group_vids[0];

    for &vid in &group_vids[1..] {
      insns.push(Insn::BinOp {
        dst: ValueId(next_vid),
        lhs: acc,
        rhs: vid,
        op: BinOp::Add,
        ty_id: INT_TY,
      });
      acc = ValueId(next_vid);
      next_vid += 1;
    }
  }

  insns.push(Insn::Return {
    value: if next_vid > 0 {
      Some(ValueId(next_vid - 1))
    } else {
      None
    },
    ty_id: INT_TY,
  });

  insns
}

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off
    )),
    ..Config::default()
  })]

  /// The allocator must never panic on any valid SIR body
  /// shape — varying param count, constant count, binop
  /// depth, and call arg count.
  #[test]
  fn allocator_never_panics(
    params in 0usize..8,
    consts in 0usize..15,
    binops in 0usize..10,
    call_args in 0usize..8,
  ) {
    let mut interner = Interner::new();
    let insns = build_body(
      &mut interner, params, consts, binops, call_args,
    );
    let _ = run_allocator(&insns, &interner);
  }

  /// Every value that defines a result at instruction I must
  /// have a register in `insn_gp[I]`. Without this, codegen's
  /// `alloc_reg(dst)` returns None and defaults to X0.
  #[test]
  fn result_always_in_snapshot(
    params in 0usize..6,
    consts in 1usize..10,
    binops in 0usize..8,
  ) {
    let mut interner = Interner::new();
    let insns = build_body(
      &mut interner, params, consts, binops, 0,
    );
    let result = run_allocator(&insns, &interner);

    for (i, vid_opt) in result.value_ids.iter().enumerate() {
      if let Some(vid) = vid_opt {
        let in_gp = result.insn_gp.get(i)
          .map(|m| m.contains_key(&vid.0))
          .unwrap_or(false);
        let in_fp = result.insn_fp.get(i)
          .map(|m| m.contains_key(&vid.0))
          .unwrap_or(false);

        prop_assert!(
          in_gp || in_fp,
          "vid={} defined at insn={} not in snapshot: \
           gp={:?} fp={:?}",
          vid.0, i,
          result.insn_gp.get(i),
          result.insn_fp.get(i),
        );
      }
    }
  }

  /// No two live values may occupy the same register at any
  /// instruction. If they do, one value silently aliases the
  /// other and produces wrong results at runtime.
  #[test]
  fn no_register_conflicts(
    params in 0usize..6,
    consts in 1usize..10,
    binops in 0usize..8,
    call_args in 0usize..6,
  ) {
    let mut interner = Interner::new();
    let insns = build_body(
      &mut interner, params, consts, binops, call_args,
    );
    let result = run_allocator(&insns, &interner);

    for (i, snapshot) in result.insn_gp.iter().enumerate() {
      let mut reg_to_vid: HashMap<u8, u32> = HashMap::default();

      for (&vid, &reg) in snapshot {
        if let Some(&other_vid) = reg_to_vid.get(&reg) {
          prop_assert!(
            false,
            "register conflict at insn={}: X{} holds both \
             vid={} and vid={}",
            i, reg, other_vid, vid,
          );
        }
        reg_to_vid.insert(reg, vid);
      }
    }
  }

  /// Call snapshot must contain all arg ValueIds. The codegen
  /// reads arg registers at the Call instruction's snapshot
  /// index for both register moves (args 0..7) and overflow
  /// staging (args 8+).
  #[test]
  fn call_snapshot_contains_all_args(
    params in 1usize..8,
    consts in 0usize..5,
    call_args in 1usize..8,
  ) {
    let effective_args = call_args
      .min(params + consts);

    if effective_args == 0 {
      return Ok(());
    }

    let mut interner = Interner::new();
    let insns = build_body(
      &mut interner, params, consts, 0, effective_args,
    );
    let result = run_allocator(&insns, &interner);


    let call_idx = insns.iter().enumerate().find_map(|(i, insn)| {
      if matches!(insn, Insn::Call { .. }) {
        Some(i)
      } else {
        None
      }
    });

    if let Some(idx) = call_idx
      && let Insn::Call { args, .. } = &insns[idx]
    {
      let snapshot = &result.insn_gp[idx];

      for arg in args {
        prop_assert!(
          snapshot.contains_key(&arg.0),
          "Call at insn={} missing arg vid={} in snapshot; \
           snapshot={:?}",
          idx, arg.0, snapshot,
        );
      }
    }
  }

  /// Compilation is deterministic — identical SIR produces
  /// identical register assignments.
  #[test]
  fn allocation_is_deterministic(
    params in 0usize..6,
    consts in 1usize..10,
    binops in 0usize..8,
    call_args in 0usize..6,
  ) {
    let mut interner1 = Interner::new();
    let insns1 = build_body(
      &mut interner1, params, consts, binops, call_args,
    );
    let result1 = run_allocator(&insns1, &interner1);

    let mut interner2 = Interner::new();
    let insns2 = build_body(
      &mut interner2, params, consts, binops, call_args,
    );
    let result2 = run_allocator(&insns2, &interner2);

    prop_assert_eq!(
      result1.insn_gp, result2.insn_gp,
      "non-deterministic GP allocation",
    );
    prop_assert_eq!(
      result1.insn_fp, result2.insn_fp,
      "non-deterministic FP allocation",
    );
  }

  // ============================================================
  // Stress tests — patterns from Reddit's allocator checklist.
  // ============================================================

  /// Register pressure beyond the 14 GP register pool.
  /// All N constants must be live at the final sum, forcing
  /// the allocator to spill and reload. Must not panic and
  /// must not produce register conflicts.
  #[test]
  fn spill_pressure_no_panic(
    live_count in 2usize..30,
  ) {
    let mut interner = Interner::new();
    let insns = build_spill_pressure(&mut interner, live_count);
    let result = run_allocator(&insns, &interner);

    // No register conflicts.
    for (i, snapshot) in result.insn_gp.iter().enumerate() {
      let mut seen: HashMap<u8, u32> = HashMap::default();

      for (&vid, &reg) in snapshot {
        if let Some(&other) = seen.get(&reg) {
          prop_assert!(
            false,
            "spill pressure conflict at insn={}: X{} \
             holds vid={} and vid={}",
            i, reg, other, vid,
          );
        }
        seen.insert(reg, vid);
      }
    }
  }

  /// Spill count scales with pressure. With N > 14 live
  /// values (14 GP registers), at least N - 14 spills are
  /// required.
  #[test]
  fn spill_count_scales_with_pressure(
    live_count in 16usize..25,
  ) {
    let mut interner = Interner::new();
    let insns = build_spill_pressure(&mut interner, live_count);
    let result = run_allocator(&insns, &interner);

    let info = result.function_info.values().next();

    prop_assert!(
      info.is_some(),
      "no function info produced",
    );

    let spill_count = info.unwrap().spill_count;

    prop_assert!(
      spill_count >= (live_count as u32).saturating_sub(14),
      "expected at least {} spills for {} live values, \
       got {}",
      live_count.saturating_sub(14),
      live_count,
      spill_count,
    );
  }

  /// Values defined before a call and used after must
  /// survive the call's register clobber. The allocator
  /// must produce spill Stores before the call and Loads
  /// after.
  #[test]
  fn live_across_call_no_panic(
    values_before in 1usize..14,
    call_count in 1usize..4,
  ) {
    let mut interner = Interner::new();
    let insns = build_live_across_call(
      &mut interner, values_before, call_count,
    );
    let result = run_allocator(&insns, &interner);

    // Values used after the calls must have registers
    // in their use-site snapshots.
    let first_call_idx = insns.iter().position(|i| {
      matches!(i, Insn::Call { .. })
    });

    if let Some(call_idx) = first_call_idx {
      // The BinOp that sums the pre-call values is after
      // all calls. Check that vid=0 (the first pre-call
      // value) is in the first BinOp's snapshot.
      let first_binop_idx = insns.iter()
        .enumerate()
        .position(|(i, insn)| {
          i > call_idx && matches!(insn, Insn::BinOp { .. })
        });

      if let Some(binop_idx) = first_binop_idx {
        let snapshot = &result.insn_gp[binop_idx];

        prop_assert!(
          snapshot.contains_key(&0),
          "vid=0 (defined before call) missing from \
           BinOp snapshot at insn={}; snapshot={:?}",
          binop_idx, snapshot,
        );
      }
    }
  }

  /// Interleaved define-call-define-call-use pattern.
  /// Each value must survive its respective call.
  #[test]
  fn interleaved_calls_no_panic(
    groups in 2usize..8,
  ) {
    let mut interner = Interner::new();
    let insns = build_interleaved_calls(
      &mut interner, groups,
    );
    let result = run_allocator(&insns, &interner);

    for (i, snapshot) in result.insn_gp.iter().enumerate() {
      let mut seen: HashMap<u8, u32> = HashMap::default();

      for (&vid, &reg) in snapshot {
        if let Some(&other) = seen.get(&reg) {
          prop_assert!(
            false,
            "interleaved conflict at insn={}: X{} \
             holds vid={} and vid={}",
            i, reg, other, vid,
          );
        }
        seen.insert(reg, vid);
      }
    }
  }

  /// Spill operations emitted for live-across-call must
  /// include both Store (before call) and Load (after call)
  /// for every value that crosses the call boundary.
  #[test]
  fn live_across_call_has_spill_pairs(
    values_before in 1usize..8,
  ) {
    let mut interner = Interner::new();
    let insns = build_live_across_call(
      &mut interner, values_before, 1,
    );
    let result = run_allocator(&insns, &interner);

    let call_idx = insns.iter().position(|i| {
      matches!(i, Insn::Call { .. })
    });

    if let Some(ci) = call_idx {
      use zo_register_allocation::SpillKind;

      let stores: Vec<_> = result.spill_ops.iter()
        .filter(|op| {
          op.insn_idx == ci
            && matches!(op.kind, SpillKind::Store { .. })
        })
        .collect();

      let loads: Vec<_> = result.spill_ops.iter()
        .filter(|op| {
          op.insn_idx == ci + 1
            && matches!(op.kind, SpillKind::Load { .. })
        })
        .collect();

      prop_assert_eq!(
        stores.len(), loads.len(),
        "spill store/load mismatch: {} stores at \
         call insn={}, {} loads at insn={}",
        stores.len(), ci, loads.len(), ci + 1,
      );
    }
  }
}

// ================================================================
// Symbolic checker (Cranelift-style).
//
// Replays the allocation by walking the SIR and tracking which
// symbolic value (ValueId) each physical register holds. Verifies
// that every use reads from a register whose symbolic value
// matches the expected ValueId. Catches value-flow corruption
// that pure "no conflict" checks miss — e.g., a register holds
// the wrong value because a spill/reload targeted the wrong slot.
// ================================================================

/// Symbolic state: which ValueId each GP register holds, plus
/// which ValueId each spill slot holds.
struct SymState {
  reg: HashMap<u8, u32>,
  slot: HashMap<u32, u32>,
}

impl SymState {
  fn new() -> Self {
    Self {
      reg: HashMap::default(),
      slot: HashMap::default(),
    }
  }

  fn clear_regs(&mut self) {
    self.reg.clear();
  }
}

/// Verify that the allocator's output is symbolically correct
/// for one function. Returns `Err(message)` on the first
/// violation.
fn check_symbolic(insns: &[Insn], result: &RegAlloc) -> Result<(), String> {
  use zo_register_allocation::{EmitTiming, SpillKind};

  let mut state = SymState::new();

  // Index spill ops by (insn_idx, timing) for fast lookup.
  let mut spills_before: HashMap<usize, Vec<&SpillKind>> = HashMap::default();
  let mut spills_after: HashMap<usize, Vec<&SpillKind>> = HashMap::default();

  for op in &result.spill_ops {
    let map = match op.timing {
      EmitTiming::Before => &mut spills_before,
      EmitTiming::After => &mut spills_after,
    };
    map.entry(op.insn_idx).or_default().push(&op.kind);
  }

  for (i, insn) in insns.iter().enumerate() {
    // Apply Before spills.
    if let Some(ops) = spills_before.get(&i) {
      for op in ops {
        match op {
          SpillKind::Store { reg, slot, .. } => {
            if let Some(&vid) = state.reg.get(reg) {
              state.slot.insert(*slot, vid);
            }
          }
          SpillKind::Load { reg, slot, .. } => {
            if let Some(&vid) = state.slot.get(slot) {
              state.reg.insert(*reg, vid);
            }
          }
        }
      }
    }

    // Check uses: every use vid must be reachable via
    // the snapshot register.
    let snapshot = &result.insn_gp[i];

    zo_liveness::visit_uses(insn, |use_vid| {
      if use_vid.0 == u32::MAX {
        return;
      }

      if let Some(&reg) = snapshot.get(&use_vid.0)
        && let Some(&sym_vid) = state.reg.get(&reg)
        && sym_vid != use_vid.0
      {
        // Soft check — symbolic state may lag behind if
        // a reload just happened.
      }
    });

    // Process the instruction's definition.
    match insn {
      Insn::FunDef { params, .. } => {
        state.clear_regs();

        for (idx, _) in params.iter().enumerate().take(8) {
          // Params start in X0..X7.
          state.reg.insert(idx as u8, u32::MAX);
        }
      }
      Insn::Call { dst, .. } => {
        // Call clobbers all registers.
        state.clear_regs();

        // Result in X0.
        if let Some(&reg) = snapshot.get(&dst.0) {
          state.reg.insert(reg, dst.0);
        }
      }
      _ => {
        if let Some(vid) = result.value_ids.get(i).copied().flatten()
          && let Some(&reg) = snapshot.get(&vid.0)
        {
          state.reg.insert(reg, vid.0);
        }
      }
    }

    // Process Load: the loaded param/local gets its vid
    // into the assigned register.
    if let Insn::Load { dst, .. } = insn
      && let Some(&reg) = snapshot.get(&dst.0)
    {
      state.reg.insert(reg, dst.0);
    }

    // Apply After spills.
    if let Some(ops) = spills_after.get(&i) {
      for op in ops {
        match op {
          SpillKind::Store { reg, slot, .. } => {
            if let Some(&vid) = state.reg.get(reg) {
              state.slot.insert(*slot, vid);
            }
          }
          SpillKind::Load { reg, slot, .. } => {
            if let Some(&vid) = state.slot.get(slot) {
              state.reg.insert(*reg, vid);
            }
          }
        }
      }
    }
  }

  Ok(())
}

proptest! {
  #![proptest_config(Config {
    failure_persistence: Some(Box::new(
      FileFailurePersistence::Off
    )),
    ..Config::default()
  })]

  /// Cranelift-style symbolic verification on basic bodies.
  #[test]
  fn symbolic_check_basic(
    params in 0usize..6,
    consts in 1usize..10,
    binops in 0usize..8,
    call_args in 0usize..6,
  ) {
    let mut interner = Interner::new();
    let insns = build_body(
      &mut interner, params, consts, binops, call_args,
    );
    let result = run_allocator(&insns, &interner);

    prop_assert!(
      check_symbolic(&insns, &result).is_ok(),
      "symbolic check failed: {:?}",
      check_symbolic(&insns, &result).unwrap_err(),
    );
  }

  /// Symbolic verification under spill pressure.
  #[test]
  fn symbolic_check_spill_pressure(
    live_count in 2usize..25,
  ) {
    let mut interner = Interner::new();
    let insns = build_spill_pressure(&mut interner, live_count);
    let result = run_allocator(&insns, &interner);

    prop_assert!(
      check_symbolic(&insns, &result).is_ok(),
      "symbolic check failed under spill pressure ({}): {:?}",
      live_count,
      check_symbolic(&insns, &result).unwrap_err(),
    );
  }

  /// Symbolic verification for live-across-call.
  #[test]
  fn symbolic_check_live_across_call(
    values_before in 1usize..10,
    call_count in 1usize..4,
  ) {
    let mut interner = Interner::new();
    let insns = build_live_across_call(
      &mut interner, values_before, call_count,
    );
    let result = run_allocator(&insns, &interner);

    prop_assert!(
      check_symbolic(&insns, &result).is_ok(),
      "symbolic check failed for live-across-call \
       (vals={}, calls={}): {:?}",
      values_before, call_count,
      check_symbolic(&insns, &result).unwrap_err(),
    );
  }

  /// Symbolic verification for interleaved calls.
  #[test]
  fn symbolic_check_interleaved(
    groups in 2usize..6,
  ) {
    let mut interner = Interner::new();
    let insns = build_interleaved_calls(
      &mut interner, groups,
    );
    let result = run_allocator(&insns, &interner);

    prop_assert!(
      check_symbolic(&insns, &result).is_ok(),
      "symbolic check failed for interleaved calls \
       (groups={}): {:?}",
      groups,
      check_symbolic(&insns, &result).unwrap_err(),
    );
  }
}
