//! SIR type-invariant validator.
//!
//! Walks a completed SIR insn stream and flags any instruction
//! that violates the SIR type-consistency contract:
//!
//! 1. `BinOp` lhs / rhs / result share a `ty_id`.
//! 2. Every `Call` arg's ty_id matches the callee's declared
//!    param ty_id.
//! 3. A `Store`'s value ty matches the `Store`'s own declared
//!    ty (and, if the target was `VarDef`-declared, the
//!    `VarDef`'s ty).
//! 4. A `Return`'s value ty matches the enclosing `FunDef`'s
//!    return ty.
//! 5. No `TyId::Error` (0), `TyId::Type` (19), or
//!    `TyId::Unknown` (20) survives anywhere; `TyId::Template`
//!    (18) is only legal on `Insn::Template`.
//!
//! Design choices:
//!
//! - **Zero dependency on `TyChecker`.** Takes only `&[Insn]`.
//!   Detecting leaked `Infer` vars needs the checker's
//!   substitution map and is deferred to a later pass.
//! - **Intrinsic Calls are skipped, not flagged.** If a Call's
//!   callee has no matching `FunDef` in the insn stream (i.e.
//!   `show`, `showln`, `check`, or a stdlib/FFI symbol linked
//!   in later) we count it as `calls_skipped` rather than
//!   producing a violation. Keeps the validator useful even
//!   before cross-module SIR is merged.
//! - **Linear walk.** One pass to build maps, one pass to
//!   check. O(n) total, no quadratic behavior.
//!
//! The validator produces a [`ValidationReport`] containing
//! every violation plus informational counters. Callers decide
//! whether to error, log, or print — this module only reports.

use crate::{Insn, LoadSource};

use zo_interner::Symbol;
use zo_ty::TyId;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

/// Sentinel `TyId`s per `tychecker.rs:96–151`. Kept local so
/// the validator stays free of any `zo-ty-checker` dependency.
const TY_ID_ERROR: u32 = 0;
const TY_ID_TEMPLATE: u32 = 18;
const TY_ID_TYPE: u32 = 19;
const TY_ID_UNKNOWN: u32 = 20;

/// A single validation failure.
#[derive(Clone, Debug, PartialEq)]
pub struct Violation {
  /// Index of the offending insn in the stream.
  pub insn_index: usize,
  /// What went wrong.
  pub kind: ViolationKind,
}

/// The kinds of invariant violations the validator reports.
#[derive(Clone, Debug, PartialEq)]
pub enum ViolationKind {
  /// `BinOp` where one operand's ty_id disagrees with another,
  /// or with the op's result ty.
  BinOpWidthMismatch {
    dst: ValueId,
    lhs_ty: TyId,
    rhs_ty: TyId,
    op_ty: TyId,
  },
  /// `Call` where arg `arg_index` has ty_id `arg_ty` but the
  /// callee's declared param ty is `param_ty`.
  CallArgMismatch {
    callee: Symbol,
    arg_index: usize,
    arg_ty: TyId,
    param_ty: TyId,
  },
  /// `Store` whose value ty_id disagrees with the Store's own
  /// declared `ty_id` (local self-consistency).
  StoreValueMismatch {
    name: Symbol,
    value_ty: TyId,
    store_ty: TyId,
  },
  /// `Store` whose ty_id disagrees with the binding's earlier
  /// `VarDef.ty_id` (cross-insn consistency within one fn).
  StoreDeclMismatch {
    name: Symbol,
    store_ty: TyId,
    decl_ty: TyId,
  },
  /// `Return` whose value ty_id disagrees with the enclosing
  /// `FunDef.return_ty`.
  ReturnValueMismatch {
    fn_name: Symbol,
    value_ty: TyId,
    fn_return_ty: TyId,
  },
  /// A placeholder `TyId` (Error / Type / Unknown / misplaced
  /// Template) leaked into SIR.
  Placeholder {
    ty_id: TyId,
    /// Human-readable descriptor of where the bad id appeared.
    context: &'static str,
  },
}

/// Outcome of running [`validate`] on an insn stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationReport {
  /// Every invariant violation found, in stream order.
  pub violations: Vec<Violation>,
  /// Number of `Call` insns skipped because the callee had no
  /// local `FunDef` (intrinsics + externals). Informational.
  pub calls_skipped: usize,
}

impl ValidationReport {
  /// True iff `violations` is empty. `calls_skipped` is
  /// informational and does not count as a failure.
  pub fn is_ok(&self) -> bool {
    self.violations.is_empty()
  }
}

/// Runs every SIR type invariant against `insns` and returns a
/// report. See the module-level doc comment for the rules.
pub fn validate(insns: &[Insn]) -> ValidationReport {
  let value_types = collect_value_types(insns);
  let fun_sigs = collect_fun_sigs(insns);

  let mut report = ValidationReport::default();
  // Tracks the enclosing function's signature so `Return` can
  // compare against the fn's declared return type.
  let mut current_fn: Option<(Symbol, TyId)> = None;
  // Per-function map of local `VarDef` declarations so a later
  // `Store { name }` can compare its own `ty_id` against the
  // binding's declared `ty_id`. Reset at every `FunDef`.
  let mut local_decls: HashMap<Symbol, TyId> = HashMap::default();

  for (idx, insn) in insns.iter().enumerate() {
    // Scope housekeeping: entering a new function resets the
    // per-fn state. FunDef itself never carries a ty_id to
    // check, so we don't walk into the placeholder checks for
    // it.
    if let Insn::FunDef {
      name, return_ty, ..
    } = insn
    {
      current_fn = Some((*name, *return_ty));
      local_decls.clear();

      // Param ty placeholders are still worth flagging — a
      // generic-leaked param will poison every arg of every
      // call to this function.
      check_placeholder(&mut report, idx, *return_ty, "FunDef.return_ty");

      if let Insn::FunDef { params, .. } = insn {
        for (_, pty) in params {
          check_placeholder(&mut report, idx, *pty, "FunDef.param");
        }
      }

      continue;
    }

    check_insn(
      insn,
      idx,
      &value_types,
      &fun_sigs,
      current_fn,
      &mut local_decls,
      &mut report,
    );
  }

  report
}

/// First-pass scan: map every value-producing `dst: ValueId`
/// to its `ty_id`. Consumed by the second pass to compare
/// operand types against op types.
fn collect_value_types(insns: &[Insn]) -> HashMap<ValueId, TyId> {
  let mut out: HashMap<ValueId, TyId> = HashMap::default();

  for insn in insns {
    match insn {
      Insn::ConstInt { dst, ty_id, .. }
      | Insn::ConstFloat { dst, ty_id, .. }
      | Insn::ConstBool { dst, ty_id, .. }
      | Insn::ConstString { dst, ty_id, .. }
      | Insn::Load { dst, ty_id, .. }
      | Insn::Call { dst, ty_id, .. }
      | Insn::BinOp { dst, ty_id, .. }
      | Insn::UnOp { dst, ty_id, .. }
      | Insn::ArrayLiteral { dst, ty_id, .. }
      | Insn::ArrayIndex { dst, ty_id, .. }
      | Insn::ArrayLen { dst, ty_id, .. }
      | Insn::ArrayPop { dst, ty_id, .. }
      | Insn::TupleLiteral { dst, ty_id, .. }
      | Insn::TupleIndex { dst, ty_id, .. }
      | Insn::EnumConstruct { dst, ty_id, .. }
      | Insn::StructConstruct { dst, ty_id, .. } => {
        out.insert(*dst, *ty_id);
      }
      Insn::Cast { dst, to_ty, .. } => {
        out.insert(*dst, *to_ty);
      }
      Insn::Template { id, ty_id, .. } => {
        out.insert(*id, *ty_id);
      }
      // Concurrency insns that produce typed values.
      Insn::ChannelCreate { dst, elem_ty, .. } => {
        out.insert(*dst, *elem_ty);
      }
      Insn::ChannelRecv { dst, ty_id, .. }
      | Insn::TaskSpawn { dst, ty_id, .. }
      | Insn::TaskAwait { dst, ty_id, .. } => {
        out.insert(*dst, *ty_id);
      }
      _ => {}
    }
  }

  out
}

/// First-pass scan: map `Symbol` → callee signature
/// `(param_tys, return_ty)` for every `Insn::FunDef`. Used to
/// validate Call-site arg widths.
fn collect_fun_sigs(insns: &[Insn]) -> HashMap<Symbol, (Vec<TyId>, TyId)> {
  let mut out: HashMap<Symbol, (Vec<TyId>, TyId)> = HashMap::default();

  for insn in insns {
    if let Insn::FunDef {
      name,
      params,
      return_ty,
      ..
    } = insn
    {
      let param_tys: Vec<TyId> = params.iter().map(|(_, t)| *t).collect();

      out.insert(*name, (param_tys, *return_ty));
    }
  }

  out
}

/// Per-insn dispatch — each arm handles the rules that apply
/// to its shape. Kept separate from [`validate`]'s outer loop
/// so the match stays readable.
#[allow(clippy::too_many_arguments)]
fn check_insn(
  insn: &Insn,
  idx: usize,
  value_types: &HashMap<ValueId, TyId>,
  fun_sigs: &HashMap<Symbol, (Vec<TyId>, TyId)>,
  current_fn: Option<(Symbol, TyId)>,
  local_decls: &mut HashMap<Symbol, TyId>,
  report: &mut ValidationReport,
) {
  match insn {
    Insn::VarDef { name, ty_id, .. } => {
      local_decls.insert(*name, *ty_id);
      check_placeholder(report, idx, *ty_id, "VarDef.ty_id");
    }

    Insn::Store { name, value, ty_id } => {
      check_placeholder(report, idx, *ty_id, "Store.ty_id");

      // Cross-insn: Store ty should match the binding's VarDef.
      if let Some(&decl_ty) = local_decls.get(name)
        && decl_ty != *ty_id
      {
        report.violations.push(Violation {
          insn_index: idx,
          kind: ViolationKind::StoreDeclMismatch {
            name: *name,
            store_ty: *ty_id,
            decl_ty,
          },
        });
      }

      // Local: value being stored must carry Store's ty.
      if let Some(&value_ty) = value_types.get(value)
        && value_ty != *ty_id
      {
        report.violations.push(Violation {
          insn_index: idx,
          kind: ViolationKind::StoreValueMismatch {
            name: *name,
            value_ty,
            store_ty: *ty_id,
          },
        });
      }
    }

    Insn::Return { value, ty_id } => {
      check_placeholder(report, idx, *ty_id, "Return.ty_id");

      let Some(v) = value else { return };
      let Some(&value_ty) = value_types.get(v) else {
        return;
      };
      let Some((fn_name, fn_return_ty)) = current_fn else {
        return;
      };

      if value_ty != fn_return_ty {
        report.violations.push(Violation {
          insn_index: idx,
          kind: ViolationKind::ReturnValueMismatch {
            fn_name,
            value_ty,
            fn_return_ty,
          },
        });
      }
    }

    Insn::BinOp {
      dst,
      lhs,
      rhs,
      ty_id,
      ..
    } => {
      check_placeholder(report, idx, *ty_id, "BinOp.ty_id");

      let lhs_ty = value_types.get(lhs).copied();
      let rhs_ty = value_types.get(rhs).copied();

      // Comparison ops (`==`, `<`, `>=`, …) and logical ops
      // legitimately produce a `bool` result from non-bool
      // operands — the operand types must still match each
      // other, but they don't have to match `ty_id`. We
      // approximate by checking lhs vs rhs pairwise first;
      // if they disagree, that's a violation regardless of
      // which op it is. Only flag op-vs-operand mismatch
      // when the op's ty_id is NOT `Bool` (TyId 2) — that
      // handles the comparison-op escape hatch without
      // per-op knowledge.
      if let (Some(lt), Some(rt)) = (lhs_ty, rhs_ty) {
        let op_ty = *ty_id;
        let bool_ty = TyId(2);
        let operand_mismatch = lt != rt;
        let result_mismatch = op_ty != bool_ty && lt != op_ty;

        if operand_mismatch || result_mismatch {
          report.violations.push(Violation {
            insn_index: idx,
            kind: ViolationKind::BinOpWidthMismatch {
              dst: *dst,
              lhs_ty: lt,
              rhs_ty: rt,
              op_ty,
            },
          });
        }
      }
    }

    Insn::UnOp { ty_id, .. } => {
      check_placeholder(report, idx, *ty_id, "UnOp.ty_id");
    }

    Insn::Call {
      name, args, ty_id, ..
    } => {
      check_placeholder(report, idx, *ty_id, "Call.ty_id");

      let Some((param_tys, _)) = fun_sigs.get(name) else {
        report.calls_skipped += 1;
        return;
      };

      // Arity mismatch is a separate concern (caught in
      // tychecker). Only check the pairwise prefix so the
      // validator never indexes out of bounds.
      for (i, (arg, param_ty)) in args.iter().zip(param_tys.iter()).enumerate()
      {
        let Some(&arg_ty) = value_types.get(arg) else {
          continue;
        };

        if arg_ty != *param_ty {
          report.violations.push(Violation {
            insn_index: idx,
            kind: ViolationKind::CallArgMismatch {
              callee: *name,
              arg_index: i,
              arg_ty,
              param_ty: *param_ty,
            },
          });
        }
      }
    }

    // Remaining value-producing insns: just the placeholder check.
    Insn::ConstInt { ty_id, .. }
    | Insn::ConstFloat { ty_id, .. }
    | Insn::ConstBool { ty_id, .. }
    | Insn::ConstString { ty_id, .. }
    | Insn::ConstDef { ty_id, .. }
    | Insn::Load { ty_id, .. }
    | Insn::Directive { ty_id, .. }
    | Insn::ArrayLiteral { ty_id, .. }
    | Insn::ArrayIndex { ty_id, .. }
    | Insn::ArrayStore { ty_id, .. }
    | Insn::ArrayLen { ty_id, .. }
    | Insn::ArrayPush { ty_id, .. }
    | Insn::ArrayPop { ty_id, .. }
    | Insn::TupleLiteral { ty_id, .. }
    | Insn::TupleIndex { ty_id, .. }
    | Insn::EnumConstruct { ty_id, .. }
    | Insn::StructConstruct { ty_id, .. }
    | Insn::FieldStore { ty_id, .. } => {
      check_placeholder(report, idx, *ty_id, placeholder_context(insn));
    }

    Insn::Cast { to_ty, .. } => {
      check_placeholder(report, idx, *to_ty, "Cast.to_ty");
    }

    // Template: its own ty_id is legitimately `Template`
    // (TyId 18), so skip the placeholder check.
    Insn::Template { .. } => {}

    // EnumDef / StructDef / ArrayTyDef carry definitional ty
    // metadata; placeholder checks would false-positive on
    // the self-referential ty_id field.
    Insn::EnumDef { .. } | Insn::StructDef { .. } | Insn::ArrayTyDef { .. } => {
    }

    Insn::FunDef { .. }
    | Insn::Label { .. }
    | Insn::Jump { .. }
    | Insn::BranchIfNot { .. }
    | Insn::ModuleLoad { .. }
    | Insn::PackDecl { .. }
    | Insn::StyleSheet { .. }
    | Insn::Nop => {}

    // Concurrency carriers — placeholder check only. Full
    // invariants (send/recv ty matches channel's elem_ty,
    // TaskSpawn/TaskAwait ty matches callee signature)
    // would require tracking channel / task definitions
    // across insns; the validator treats these as opaque
    // typed carriers and trusts the executor to emit
    // well-typed operands.
    Insn::ChannelCreate { elem_ty, .. } => {
      check_placeholder(report, idx, *elem_ty, "ChannelCreate.elem_ty");
    }
    Insn::ChannelSend { ty_id, .. } => {
      check_placeholder(report, idx, *ty_id, "ChannelSend.ty_id");
    }
    Insn::ChannelRecv { ty_id, .. } => {
      check_placeholder(report, idx, *ty_id, "ChannelRecv.ty_id");
    }
    Insn::TaskSpawn { ty_id, .. } => {
      check_placeholder(report, idx, *ty_id, "TaskSpawn.ty_id");
    }
    Insn::TaskAwait { ty_id, .. } => {
      check_placeholder(report, idx, *ty_id, "TaskAwait.ty_id");
    }
    Insn::NurseryBegin { .. } | Insn::NurseryEnd { .. } => {}
    Insn::SelectWait { elem_ty, .. } => {
      check_placeholder(report, idx, *elem_ty, "SelectWait.elem_ty");
    }
  }
}

/// Emits a `Placeholder` violation iff `ty` is one of the
/// forbidden sentinel ids (`Error`, `Type`, `Unknown`). An
/// off-`Insn::Template` `Template` id is also flagged since
/// the caller only routes real Template ids through the
/// validator's Template arm (which skips this check).
fn check_placeholder(
  report: &mut ValidationReport,
  idx: usize,
  ty: TyId,
  context: &'static str,
) {
  let bad = matches!(
    ty.0,
    TY_ID_ERROR | TY_ID_TEMPLATE | TY_ID_TYPE | TY_ID_UNKNOWN
  );

  if bad {
    report.violations.push(Violation {
      insn_index: idx,
      kind: ViolationKind::Placeholder { ty_id: ty, context },
    });
  }
}

/// Short label for a value-producing insn, used as the
/// `context` of a placeholder violation.
fn placeholder_context(insn: &Insn) -> &'static str {
  match insn {
    Insn::ConstInt { .. } => "ConstInt.ty_id",
    Insn::ConstFloat { .. } => "ConstFloat.ty_id",
    Insn::ConstBool { .. } => "ConstBool.ty_id",
    Insn::ConstString { .. } => "ConstString.ty_id",
    Insn::ConstDef { .. } => "ConstDef.ty_id",
    Insn::Load { .. } => "Load.ty_id",
    Insn::Directive { .. } => "Directive.ty_id",
    Insn::ArrayLiteral { .. } => "ArrayLiteral.ty_id",
    Insn::ArrayIndex { .. } => "ArrayIndex.ty_id",
    Insn::ArrayStore { .. } => "ArrayStore.ty_id",
    Insn::ArrayLen { .. } => "ArrayLen.ty_id",
    Insn::ArrayPush { .. } => "ArrayPush.ty_id",
    Insn::ArrayPop { .. } => "ArrayPop.ty_id",
    Insn::TupleLiteral { .. } => "TupleLiteral.ty_id",
    Insn::TupleIndex { .. } => "TupleIndex.ty_id",
    Insn::EnumConstruct { .. } => "EnumConstruct.ty_id",
    Insn::StructConstruct { .. } => "StructConstruct.ty_id",
    Insn::FieldStore { .. } => "FieldStore.ty_id",
    _ => "unknown",
  }
}

// `LoadSource` isn't used here, but re-exported at module
// level for future arms that may want to discriminate on the
// load source (e.g., param vs local) without importing
// `zo-sir::LoadSource` separately. Kept as a compile-time
// reachability assertion.
const _: fn(&LoadSource) = |_| {};

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{BinOp, NurseryKind, SpawnKind};
  use zo_value::FunctionKind;

  fn vid(n: u32) -> ValueId {
    ValueId(n)
  }

  fn sym(n: u32) -> Symbol {
    Symbol(n)
  }

  #[test]
  fn clean_stream_has_no_violations() {
    let insns = vec![
      Insn::ConstInt {
        dst: vid(0),
        value: 42,
        ty_id: TyId(8),
      },
      Insn::ConstInt {
        dst: vid(1),
        value: 7,
        ty_id: TyId(8),
      },
      Insn::BinOp {
        dst: vid(2),
        op: BinOp::Add,
        lhs: vid(0),
        rhs: vid(1),
        ty_id: TyId(8),
      },
    ];

    let report = validate(&insns);

    assert!(report.is_ok(), "{report:?}");
  }

  #[test]
  fn binop_width_mismatch_is_flagged() {
    let insns = vec![
      Insn::ConstInt {
        dst: vid(0),
        value: 42,
        ty_id: TyId(8), // s32
      },
      Insn::ConstInt {
        dst: vid(1),
        value: 7,
        ty_id: TyId(9), // s64
      },
      Insn::BinOp {
        dst: vid(2),
        op: BinOp::Add,
        lhs: vid(0),
        rhs: vid(1),
        ty_id: TyId(9),
      },
    ];

    let report = validate(&insns);

    assert_eq!(report.violations.len(), 1);
    assert!(matches!(
      report.violations[0].kind,
      ViolationKind::BinOpWidthMismatch { .. }
    ));
  }

  #[test]
  fn binop_comparison_bool_result_is_ok() {
    // `s32 < s32 -> bool` is a legitimate BinOp, even though
    // lhs/rhs ty (s32) doesn't equal op ty (bool).
    let insns = vec![
      Insn::ConstInt {
        dst: vid(0),
        value: 1,
        ty_id: TyId(8),
      },
      Insn::ConstInt {
        dst: vid(1),
        value: 2,
        ty_id: TyId(8),
      },
      Insn::BinOp {
        dst: vid(2),
        op: BinOp::Lt,
        lhs: vid(0),
        rhs: vid(1),
        ty_id: TyId(2), // bool
      },
    ];

    let report = validate(&insns);

    assert!(report.is_ok(), "{report:?}");
  }

  #[test]
  fn call_arg_width_mismatch_is_flagged() {
    // fun f(x: s64) { ... }
    // f(42 : s32)  — s32 into s64 param.
    let insns = vec![
      Insn::FunDef {
        name: sym(1),
        params: vec![(sym(2), TyId(9))], // s64
        return_ty: TyId(1),              // unit
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: zo_value::Pubness::No,
      },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
      Insn::ConstInt {
        dst: vid(0),
        value: 42,
        ty_id: TyId(8), // s32
      },
      Insn::Call {
        dst: vid(1),
        name: sym(1),
        args: vec![vid(0)],
        ty_id: TyId(1),
      },
    ];

    let report = validate(&insns);

    assert_eq!(report.violations.len(), 1);
    assert!(matches!(
      report.violations[0].kind,
      ViolationKind::CallArgMismatch { .. }
    ));
  }

  #[test]
  fn missing_callee_increments_calls_skipped() {
    // Calling `show` — no matching FunDef in the stream.
    let insns = vec![
      Insn::ConstString {
        dst: vid(0),
        symbol: sym(5),
        ty_id: TyId(4),
      },
      Insn::Call {
        dst: vid(1),
        name: sym(99), // "show"
        args: vec![vid(0)],
        ty_id: TyId(1),
      },
    ];

    let report = validate(&insns);

    assert!(report.is_ok());
    assert_eq!(report.calls_skipped, 1);
  }

  #[test]
  fn placeholder_error_id_is_flagged() {
    let insns = vec![Insn::ConstInt {
      dst: vid(0),
      value: 0,
      ty_id: TyId(TY_ID_ERROR),
    }];

    let report = validate(&insns);

    assert_eq!(report.violations.len(), 1);
    assert!(matches!(
      report.violations[0].kind,
      ViolationKind::Placeholder { .. }
    ));
  }

  #[test]
  fn store_decl_mismatch_is_flagged() {
    // imu x: s32 = 42;  — VarDef ty s32.
    // x = (42 : s64);   — Store ty s64. Disagrees with VarDef.
    let insns = vec![
      Insn::VarDef {
        name: sym(1),
        ty_id: TyId(8),
        init: None,
        mutability: zo_ty::Mutability::Yes,
        pubness: zo_value::Pubness::No,
      },
      Insn::ConstInt {
        dst: vid(0),
        value: 42,
        ty_id: TyId(9),
      },
      Insn::Store {
        name: sym(1),
        value: vid(0),
        ty_id: TyId(9),
      },
    ];

    let report = validate(&insns);

    // Both the decl mismatch and the value mismatch fire —
    // but the ValueMismatch one won't (value_ty == store_ty).
    // Expect exactly one: StoreDeclMismatch.
    assert_eq!(report.violations.len(), 1);
    assert!(matches!(
      report.violations[0].kind,
      ViolationKind::StoreDeclMismatch { .. }
    ));
  }

  // ===== PHASE 3: CONCURRENCY INSNS =====

  #[test]
  fn nursery_scoped_channel_spawn_await_is_clean() {
    // nursery { (tx, rx) := channel(); spawn prod(tx);
    //           imu v := rx.recv(); await task; }
    let elem_ty = TyId(8); // s32
    let task_ty = TyId(30); // Ty::Task(unit), arbitrary id
    let insns = vec![
      Insn::NurseryBegin {
        label: 1,
        kind: NurseryKind::Scoped,
      },
      Insn::ChannelCreate {
        dst: vid(0),
        elem_ty,
        capacity: 0,
      },
      Insn::TaskSpawn {
        dst: vid(2),
        callee: sym(1),
        args: vec![vid(0)],
        ty_id: task_ty,
        kind: SpawnKind::Green,
      },
      Insn::ChannelRecv {
        dst: vid(3),
        channel: vid(0),
        ty_id: elem_ty,
      },
      Insn::TaskAwait {
        dst: vid(4),
        task: vid(2),
        ty_id: TyId(1), // unit
      },
      Insn::NurseryEnd { label: 1 },
    ];

    let report = validate(&insns);

    assert!(report.is_ok(), "{report:?}");
  }

  #[test]
  fn channel_send_with_placeholder_ty_is_flagged() {
    // ChannelSend.ty_id == TyId::Error (0) should trip the
    // placeholder check exactly like any other carrier.
    let insns = vec![Insn::ChannelSend {
      channel: vid(0),
      value: vid(1),
      ty_id: TyId(0), // forbidden sentinel
    }];

    let report = validate(&insns);

    assert!(!report.is_ok());
    assert!(
      report
        .violations
        .iter()
        .any(|v| matches!(v.kind, ViolationKind::Placeholder { .. })),
      "expected Placeholder violation, got {report:?}"
    );
  }

  #[test]
  fn channel_create_registers_both_halves_in_value_types() {
    // Regression for `collect_value_types` on the
    // single-dst `ChannelCreate` — a downstream BinOp
    // using the chan handle needs to see its elem_ty
    // for width-parity checks.
    let elem_ty = TyId(8);
    let insns = vec![
      Insn::ChannelCreate {
        dst: vid(0),
        elem_ty,
        capacity: 4,
      },
      Insn::BinOp {
        dst: vid(2),
        op: BinOp::Eq,
        lhs: vid(0),
        rhs: vid(0),
        ty_id: TyId(2), // bool — the validator's comparison escape hatch
      },
    ];

    let report = validate(&insns);

    // Both lhs (tx) and rhs (rx) resolve to elem_ty via the
    // collector, so the BinOp width check passes.
    assert!(report.is_ok(), "{report:?}");
  }
}
