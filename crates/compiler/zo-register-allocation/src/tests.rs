use crate::RegAlloc;
use crate::allocator::{AllocCtx, allocate_function};

use zo_interner::Interner;
use zo_liveness::compute_value_ids;
use zo_sir::Insn;
use zo_span::Span;
use zo_ty::TyId;
use zo_value::{FunctionKind, Pubness, ValueId};

use rustc_hash::FxHashMap as HashMap;

/// Build a minimal `RegAlloc` with per-insn snapshot
/// vectors sized for `n` instructions.
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

const INT_TY: TyId = TyId(10);
const VOID_TY: TyId = TyId(1);

fn fundef(
  interner: &mut Interner,
  name: &str,
  params: &[&str],
  body_start: u32,
) -> Insn {
  let sym = interner.intern(name);
  let ps: Vec<_> = params
    .iter()
    .map(|p| (interner.intern(p), INT_TY))
    .collect();

  Insn::FunDef {
    name: sym,
    params: ps,
    return_ty: INT_TY,
    body_start,
    kind: FunctionKind::UserDefined,
    pubness: Pubness::No,
    mut_self: false,
    link_name: None,
    owning_pack: None,
    span: Span::ZERO,
    is_test: false,
  }
}

/// Pre-call snapshot must contain arg values.
///
/// Regression: the allocator used to take the Call
/// snapshot AFTER `clear_all`, which wiped all register
/// state. The codegen's overflow-arg staging loop reads
/// `alloc_reg(arg)` at the Call's snapshot index — if
/// the arg isn't there, it falls through to a stale
/// fallback or returns None (wrong register / X0 default).
#[test]
fn call_snapshot_contains_args() {
  let mut interner = Interner::new();

  let callee_sym = interner.intern("callee");

  // SIR: fun caller(a) { callee(a); }
  //   [0] FunDef caller(a)
  //   [1] Load a from Param(0)
  //   [2] Call callee(a) -> dst
  //   [3] Return void
  let insns = vec![
    fundef(&mut interner, "caller", &["a"], 1),
    Insn::Load {
      dst: ValueId(0),
      src: zo_sir::LoadSource::Param(0),
      ty_id: INT_TY,
    },
    Insn::Call {
      dst: ValueId(1),
      name: callee_sym,
      callee_pack: None,
      args: vec![ValueId(0)],
      ty_id: INT_TY,
    },
    Insn::Return {
      value: None,
      ty_id: VOID_TY,
    },
  ];

  let value_ids = compute_value_ids(&insns);
  let n = insns.len();
  let mut result = empty_result(n);
  result.value_ids = value_ids.clone();

  let struct_return_fns = HashMap::default();

  let ctx = AllocCtx {
    insns: &insns,
    start: 0,
    end: n,
    value_ids: &value_ids,
    num_values: 2,
    interner: &interner,
    struct_return_fns: &struct_return_fns,
  };

  allocate_function(&ctx, &mut result);

  // The snapshot at the Call instruction (index 2) must
  // contain the arg ValueId(0) — the codegen reads it
  // during overflow-arg staging and register moves.
  let call_snapshot = &result.insn_gp[2];

  assert!(
    call_snapshot.contains_key(&0),
    "Call snapshot must contain arg vid=0; got {:?}",
    call_snapshot,
  );
}

/// General-case snapshot must contain reloaded uses.
///
/// After a Call spills and reloads values, the next
/// instruction's snapshot must include those reloaded
/// values — the codegen needs them for operand lookups.
#[test]
fn post_call_snapshot_contains_reloaded_values() {
  let mut interner = Interner::new();

  let callee_sym = interner.intern("callee");

  // SIR: fun f(a) { imu b = callee(a); b + 1; }
  //   [0] FunDef f(a)
  //   [1] Load a from Param(0)
  //   [2] Call callee(a) -> b
  //   [3] ConstInt 1 -> c
  //   [4] BinOp b + c -> d
  //   [5] Return d
  let insns = vec![
    fundef(&mut interner, "f", &["a"], 1),
    Insn::Load {
      dst: ValueId(0),
      src: zo_sir::LoadSource::Param(0),
      ty_id: INT_TY,
    },
    Insn::Call {
      dst: ValueId(1),
      name: callee_sym,
      callee_pack: None,
      args: vec![ValueId(0)],
      ty_id: INT_TY,
    },
    Insn::ConstInt {
      dst: ValueId(2),
      value: 1,
      ty_id: INT_TY,
    },
    Insn::BinOp {
      dst: ValueId(3),
      lhs: ValueId(1),
      rhs: ValueId(2),
      op: zo_sir::BinOp::Add,
      ty_id: INT_TY,
    },
    Insn::Return {
      value: Some(ValueId(3)),
      ty_id: INT_TY,
    },
  ];

  let value_ids = compute_value_ids(&insns);
  let n = insns.len();
  let mut result = empty_result(n);
  result.value_ids = value_ids.clone();

  let struct_return_fns = HashMap::default();

  let ctx = AllocCtx {
    insns: &insns,
    start: 0,
    end: n,
    value_ids: &value_ids,
    num_values: 4,
    interner: &interner,
    struct_return_fns: &struct_return_fns,
  };

  allocate_function(&ctx, &mut result);

  // The Call result (vid=1) must be in the snapshot at
  // instruction 4 (BinOp) — it's a use of that insn.
  let binop_snapshot = &result.insn_gp[4];

  assert!(
    binop_snapshot.contains_key(&1),
    "BinOp snapshot must contain Call result vid=1; got {:?}",
    binop_snapshot,
  );
}

/// Result allocation must be visible in the snapshot at
/// the defining instruction. Without `snapshot_result`,
/// `alloc_reg(dst)` would return None for the instruction
/// that just allocated the register.
#[test]
fn result_visible_in_own_snapshot() {
  let mut interner = Interner::new();

  // SIR: fun f() { 42; }
  //   [0] FunDef f()
  //   [1] ConstInt 42 -> vid=0
  //   [2] Return void
  let insns = vec![
    fundef(&mut interner, "f", &[], 1),
    Insn::ConstInt {
      dst: ValueId(0),
      value: 42,
      ty_id: INT_TY,
    },
    Insn::Return {
      value: None,
      ty_id: VOID_TY,
    },
  ];

  let value_ids = compute_value_ids(&insns);
  let n = insns.len();
  let mut result = empty_result(n);
  result.value_ids = value_ids.clone();

  let struct_return_fns = HashMap::default();

  let ctx = AllocCtx {
    insns: &insns,
    start: 0,
    end: n,
    value_ids: &value_ids,
    num_values: 1,
    interner: &interner,
    struct_return_fns: &struct_return_fns,
  };

  allocate_function(&ctx, &mut result);

  let const_snapshot = &result.insn_gp[1];

  assert!(
    const_snapshot.contains_key(&0),
    "ConstInt snapshot must contain its own result vid=0; got {:?}",
    const_snapshot,
  );
}
