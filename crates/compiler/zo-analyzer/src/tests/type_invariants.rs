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

/// `x + 5` where `x: u16` — the `x` `Load` is `u16` but the
/// `5` literal is emitted as `s32`. The executor's binop
/// unification fails and it **silently aborts** — no `BinOp`
/// insn reaches SIR, and the enclosing `VarDef` has
/// `init: None`.
///
/// Today this test pins down the silent drop. **Plan Phase 3**
/// (BinOp operand propagation) + **Phase 5** (diagnostics)
/// flip this to: the `BinOp` is emitted with both operands
/// `u16`, `init: Some(_)`.
#[test]
fn binop_u16_plus_bare_literal_silently_drops_binop_today() {
  let source = r"
    fun main() {
      imu x: u16 = 10;
      imu _y: u16 = x + 5;
    }
  ";

  let (semantic, _) = analyze_and_validate(source);

  let has_binop = semantic
    .sir
    .instructions
    .iter()
    .any(|insn| matches!(insn, Insn::BinOp { .. }));

  // Today: no BinOp is emitted. The test MUST fail once the
  // executor starts emitting the BinOp correctly — at that
  // point flip the assertion to `assert!(has_binop)`.
  assert!(
    !has_binop,
    "unexpected BinOp in SIR — did an earlier phase land? \
     Flip this assertion and check the operand widths.",
  );

  let init_is_none = semantic
    .sir
    .instructions
    .iter()
    .any(|insn| matches!(insn, Insn::VarDef { init: None, .. }));

  assert!(
    init_is_none,
    "expected a `VarDef {{ init: None }}` from the silent drop",
  );
}

/// `f(42)` where `f(x: s64)` — the `42` literal is emitted
/// as `s32` and the callee's param is `s64`. The executor's
/// call unification fails and **silently aborts** — no
/// `Call` insn reaches SIR.
///
/// Today this test pins down the silent drop. **Plan Phase 2**
/// (Call arg propagation) + **Phase 5** (diagnostics) flip
/// this to: the `Call` is emitted with the arg as `s64`.
#[test]
fn call_with_s64_param_and_bare_literal_silently_drops_call_today() {
  let source = r"
    fun f(x: s64) -> s64 {
      return x;
    }

    fun main() {
      imu _y: s64 = f(42);
    }
  ";

  let (semantic, _) = analyze_and_validate(source);

  let has_call = semantic
    .sir
    .instructions
    .iter()
    .any(|insn| matches!(insn, Insn::Call { .. }));

  assert!(
    !has_call,
    "unexpected Call in SIR — did an earlier phase land? \
     Flip this assertion and check the arg widths.",
  );
}
