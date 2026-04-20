//! Integration tests pinning down the *current* SIR behavior
//! for int/float literal typing.
//!
//! Two kinds of failure today:
//!
//! 1. **Surviving mismatch** — the executor emits an insn
//!    whose ty_id disagrees with context. The validator
//!    catches it (e.g. `Return` below).
//! 2. **Silent drop** — the executor hits a mismatch and
//!    aborts emission of the enclosing expression. No insn
//!    in SIR, no diagnostic, program "compiles" with an
//!    empty body (e.g. `BinOp` and `Call` below).
//!
//! The validator only helps with kind 1. Kind 2 needs the
//! executor to emit diagnostics (plan Phase 5) — and then
//! Phases 1–4 make both kinds unreachable because literals
//! adopt their context type from the start.
//!
//! Every test here is an acceptance gate. When a phase lands
//! that makes the scenario emit clean SIR, flip the assertion.

use super::common::analyze_and_validate;

use zo_sir::{Insn, ViolationKind};

/// Baseline — a program whose literal widths are already
/// context-matched produces clean SIR today. Guards against
/// the validator having false positives on good code.
#[test]
fn clean_s32_decl_has_no_violations() {
  let source = r"
    fun main() {
      imu x: s32 = 42;
    }
  ";

  let (_, report) = analyze_and_validate(source);

  assert!(
    report.is_ok(),
    "expected clean SIR, got violations: {:#?}",
    report.violations,
  );
}

/// `return 42;` in `-> s64` — the literal keeps ty_id `s32`
/// and the enclosing fn's return ty is `s64`. The `Return`
/// insn survives with the mismatch, so the validator catches
/// it.
///
/// **Plan Phase 4** (Return / Cast propagation) flips this to
/// `is_ok()`.
#[test]
fn return_bare_literal_from_s64_fn_trips_return_mismatch() {
  let source = r"
    fun get() -> s64 {
      return 42;
    }

    fun main() {
      imu _x: s64 = get();
    }
  ";

  let (_, report) = analyze_and_validate(source);

  let found = report
    .violations
    .iter()
    .any(|v| matches!(v.kind, ViolationKind::ReturnValueMismatch { .. }));

  assert!(
    found,
    "expected at least one ReturnValueMismatch, got: {:#?}",
    report.violations,
  );
}

/// `x + 5` where `x: u16`, all wrapped in `imu _y: u16 =
/// ...`. **Plan Phase 1** fixed this: the decl-site push of
/// `u16` onto `expected_ty_stack` propagates through the
/// init expression, so the `5` literal lands with `ty_id:
/// u16`, the BinOp unifies cleanly, and SIR is valid.
#[test]
fn binop_u16_plus_literal_in_u16_decl_emits_clean_binop() {
  let source = r"
    fun main() {
      imu x: u16 = 10;
      imu _y: u16 = x + 5;
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let binop = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::BinOp { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert!(
    binop.is_some(),
    "expected a BinOp insn in SIR (Phase 1 should have \
     emitted one); saw: {:#?}",
    semantic.sir.instructions,
  );
  assert_eq!(
    binop.unwrap().0,
    12,
    "expected BinOp.ty_id == u16 (TyId 12); see `PLAN_SIR_TYPE_INVARIANTS.md` Phase 1",
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 1's clean SIR; got: {:#?}",
    report.violations,
  );
}

/// `f(42)` where `f(x: s64)`, all wrapped in `imu _y: s64 =
/// ...`. **Plan Phase 1** fixed this: the decl's `s64`
/// context flows down into the call arg evaluation, so `42`
/// lands with `ty_id: s64` and unification succeeds.
#[test]
fn call_s64_arg_from_literal_in_s64_decl_emits_clean_call() {
  let source = r"
    fun f(x: s64) -> s64 {
      return x;
    }

    fun main() {
      imu _y: s64 = f(42);
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let call_ty = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::Call { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert!(
    call_ty.is_some(),
    "expected a Call insn in SIR (Phase 1 should have \
     emitted one); saw: {:#?}",
    semantic.sir.instructions,
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 1's clean SIR; got: {:#?}",
    report.violations,
  );
}
