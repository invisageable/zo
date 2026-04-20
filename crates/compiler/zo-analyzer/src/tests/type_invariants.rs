//! Integration tests pinning down the SIR produced by real
//! zo source for the int/float literal typing scenarios
//! driven by `PLAN_SIR_TYPE_INVARIANTS.md`.
//!
//! Each test represents a scenario that once either silently
//! dropped or emitted mixed-width SIR; after Phases 1–4, the
//! executor produces clean SIR for all of them. The tests
//! assert on both the emitted insns (right widths) and the
//! validator report (no invariant violations).
//!
//! If a future change regresses expected-type propagation,
//! these fail with a clear message naming the expected width
//! and the phase the scenario belongs to.

use super::common::analyze_and_validate;

use zo_error::ErrorKind;
use zo_reporter::collect_errors;
use zo_sir::Insn;

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

/// `return 42;` in `-> s64`. **Plan Phase 4** fixed this:
/// `execute_return` pushes the fn's `return_ty` onto
/// `expected_ty_stack`, so the literal `42` adopts `s64`
/// and the `Return` insn emits with matching widths.
#[test]
fn return_bare_literal_in_s64_fn_adopts_return_ty() {
  let source = r"
    fun get() -> s64 {
      return 42;
    }

    fun main() {
      imu _x: s64 = get();
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let const_int = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::ConstInt { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert!(
    const_int.is_some(),
    "expected a ConstInt for the literal 42 in the return",
  );
  assert_eq!(
    const_int.unwrap().0,
    9,
    "ConstInt.ty_id should be s64 (TyId 9); Phase 4 \
     `execute_return` should have pushed s64 as expected",
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 4's clean SIR; got: {:#?}",
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

/// **Generic-mono narrow pass** (post-plan follow-up) —
/// `identity<$T>(42)` where `$T` resolves to `int` via
/// `imu x: int = identity(42);`. Without the global
/// ty-id resolve walker, the mono'd `identity__int`
/// FunDef's param `TyId` stays as the raw `$T` value
/// (in the interned-aggregate range), which codegen
/// then maps to `ptr` (I64) — producing a signature
/// mismatch against the caller's `ConstInt(42, s32)`.
/// The resolve walker rewrites both sides to their
/// concrete `TyId` so the validator and codegen agree.
#[test]
fn generic_identity_call_has_clean_sir() {
  let source = r"
    fun identity<$T>(x: $T) -> $T { x }

    fun main() {
      imu _a: int = identity(42);
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let mono_fundef = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::FunDef {
      params, return_ty, ..
    } = insn
      && params.len() == 1
    {
      Some((params[0].1, *return_ty))
    } else {
      None
    }
  });

  assert!(
    mono_fundef.is_some(),
    "expected the mono'd identity FunDef in SIR",
  );

  let (param_ty, return_ty) = mono_fundef.unwrap();

  // Both should resolve to s32 (TyId 8) per D1 — the
  // mono'd `identity__int` with `int = s32`.
  assert_eq!(
    param_ty.0, 8,
    "mono'd FunDef param should be s32 (TyId 8), not \
     a raw generic $T; got TyId {}",
    param_ty.0,
  );
  assert_eq!(
    return_ty.0, 8,
    "mono'd FunDef return_ty should be s32 (TyId 8); \
     got TyId {}",
    return_ty.0,
  );

  assert!(
    report.is_ok(),
    "validator should accept the resolved SIR; got: {:#?}",
    report.violations,
  );
}

/// **Phase 7** — broad regression coverage. Compiles a
/// handful of representative zo programs (literal decls,
/// calls, binops, returns, float paths, nested contexts)
/// and asserts the SIR validator is clean for all of them.
///
/// If a future change to the executor / tychecker
/// accidentally re-introduces mixed-width SIR anywhere in
/// these shapes, this test fails immediately. Cheaper than
/// wiring the validator into the release-mode compile
/// pipeline (see Phase 7 finding — measured cost would
/// double compile time for a 1000-line program, well over
/// the plan's 1% threshold).
#[test]
fn phase_7_broad_invariant_regression_coverage() {
  let programs: &[(&str, &str)] = &[
    ("int decl", r"fun main() { imu _x: s32 = 42; }"),
    ("float decl", r"fun main() { imu _x: f32 = 3.14; }"),
    (
      "bare call + arg",
      r"fun f(x: u64) -> u64 { return x; }
        fun main() { f(42); }",
    ),
    (
      "nested call",
      r"fun g(x: s64) -> s64 { return x; }
        fun f(x: s64) -> s64 { return x; }
        fun main() { f(g(42)); }",
    ),
    (
      "bare binop, RHS literal",
      r"fun main() { mut x: u16 = 10; x = x + 32; }",
    ),
    (
      "bare binop, LHS literal",
      r"fun main() { mut x: u16 = 10; x = 32 + x; }",
    ),
    (
      "return expression",
      r"fun f(a: u16, b: u16) -> u16 { return a + b + 1; }
        fun main() { imu _x: u16 = f(1, 2); }",
    ),
    (
      "float binop chain",
      r"fun main() {
          imu a: f32 = 1.0;
          imu _b: f32 = a + 0.5;
          imu _c: f32 = 0.25 + a;
        }",
    ),
  ];

  for (label, source) in programs {
    let (_, report) = analyze_and_validate(source);

    assert!(
      report.is_ok(),
      "Phase 7 regression: [{label}] produced SIR violations: \
       {:#?}",
      report.violations,
    );
  }
}

/// **Phase 6** — `imu x: f32 = 3.14;` — the float literal
/// reads `Some(f32)` from `peek_expected_float_ty` at
/// emission time (via the Phase 1 decl push) and lands with
/// `ty_id: f32` directly. `narrow_float_literal` in
/// `finalize_pending_decl` covers any edge cases where
/// emission went the default path.
#[test]
fn float_decl_f32_adopts_annotation() {
  let source = r"
    fun main() {
      imu _x: f32 = 3.14;
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let const_float = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::ConstFloat { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert!(
    const_float.is_some(),
    "expected a ConstFloat in SIR for `3.14`",
  );
  assert_eq!(
    const_float.unwrap().0,
    15,
    "ConstFloat.ty_id should be f32 (TyId 15); Phase 6",
  );

  assert!(
    report.is_ok(),
    "validator should accept clean SIR; got: {:#?}",
    report.violations,
  );
}

/// **Phase 6** — `return 3.14;` in `fn -> f32` walks through
/// Phase 4's `execute_return` push (Some(f32)) so the
/// literal adopts f32 at emission. No narrow fallback
/// needed.
#[test]
fn float_return_literal_in_f32_fn_adopts_return_ty() {
  let source = r"
    fun get() -> f32 {
      return 3.14;
    }

    fun main() {
      imu _x: f32 = get();
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let const_float = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::ConstFloat { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert_eq!(
    const_float.map(|t| t.0),
    Some(15),
    "ConstFloat.ty_id should be f32 (TyId 15); Phase 6 via Phase 4",
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 6's clean SIR; got: {:#?}",
    report.violations,
  );
}

/// **Phase 5** — a binop with two *concrete* mismatched
/// operands (no literal to narrow) must produce a real
/// `TypeMismatch` diagnostic, not silently drop. Before
/// Phase 5 this program compiled with no BinOp in SIR and
/// no error message.
#[test]
fn binop_concrete_width_mismatch_reports_type_mismatch() {
  let source = r"
    fun main() {
      imu x: u16 = 1;
      imu y: u32 = 2;
      imu _z: u32 = x + y;
    }
  ";

  let (_, _) = analyze_and_validate(source);

  let errors = collect_errors();
  let has_mismatch = errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch);

  assert!(
    has_mismatch,
    "expected a TypeMismatch error for `u16 + u32`; saw: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>(),
  );
}

/// **Phase 5** — a Call with a concrete arg whose type
/// disagrees with the concrete param type must report
/// `TypeMismatch`.
#[test]
fn call_concrete_arg_mismatch_reports_type_mismatch() {
  let source = r"
    fun f(x: u16) -> u16 {
      return x;
    }

    fun main() {
      imu y: u32 = 10;
      imu _z: u16 = f(y);
    }
  ";

  let (_, _) = analyze_and_validate(source);

  let errors = collect_errors();
  let has_mismatch = errors.iter().any(|e| e.kind() == ErrorKind::TypeMismatch);

  assert!(
    has_mismatch,
    "expected a TypeMismatch error for `f(y: u32)` with `f(x: u16)`; saw: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>(),
  );
}

/// Assignment-RHS binop: `x = x + 42;` where `x: u16`. No
/// enclosing typed decl covers the RHS — Phases 1 and 2
/// don't help. **Plan Phase 3** pushes the LHS's type
/// (`u16`) onto `expected_ty_stack` at defer-time, so the
/// RHS literal `42` adopts `u16`.
#[test]
fn assign_rhs_binop_rhs_literal_adopts_lhs_ty() {
  let source = r"
    fun main() {
      mut x: u16 = 10;
      x = x + 42;
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let binop_ty = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::BinOp { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert!(
    binop_ty.is_some(),
    "expected a BinOp for `x + 42` (Phase 3); saw: {:#?}",
    semantic.sir.instructions,
  );
  assert_eq!(
    binop_ty.unwrap().0,
    12,
    "BinOp.ty_id should be u16 (TyId 12)",
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 3's clean SIR; got: {:#?}",
    report.violations,
  );
}

/// LHS-literal binop: `x = 42 + x;` where `x: u16`. The
/// literal is emitted BEFORE the `+` fires, so the defer-
/// path push can't help. **Plan Phase 3**'s post-hoc narrow
/// rewrites the default-typed `ConstInt.ty_id` in place to
/// match the concrete RHS before unification runs.
#[test]
fn assign_rhs_binop_lhs_literal_narrows_to_rhs_ty() {
  let source = r"
    fun main() {
      mut x: u16 = 10;
      x = 42 + x;
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let binop_ty = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::BinOp { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert!(
    binop_ty.is_some(),
    "expected a BinOp for `42 + x` (Phase 3); saw: {:#?}",
    semantic.sir.instructions,
  );
  assert_eq!(
    binop_ty.unwrap().0,
    12,
    "BinOp.ty_id should be u16 (TyId 12)",
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 3's clean SIR; got: {:#?}",
    report.violations,
  );
}

/// Bare statement call `f(42);` — no enclosing typed decl
/// to carry context. **Plan Phase 2** fixed this: the Call
/// itself primes `expected_ty_stack` with the callee's param
/// types before each arg evaluates, so `42` adopts `s64`
/// from the param signature directly.
#[test]
fn bare_call_s64_arg_from_literal_emits_clean_call() {
  let source = r"
    fun f(x: s64) -> s64 {
      return x;
    }

    fun main() {
      f(42);
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let call_exists = semantic
    .sir
    .instructions
    .iter()
    .any(|insn| matches!(insn, Insn::Call { .. }));

  assert!(
    call_exists,
    "expected a Call insn in SIR for `f(42);` (Phase 2); saw: {:#?}",
    semantic.sir.instructions,
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 2's clean SIR; got: {:#?}",
    report.violations,
  );
}

/// Nested call `f(g(42))` — outer arg is a call expression,
/// inner arg is the literal. **Plan Phase 2** primes the
/// inner call's context independently, so `42` adopts `g`'s
/// param type (not `f`'s).
#[test]
fn nested_call_literal_adopts_inner_callee_param_ty() {
  let source = r"
    fun g(x: s64) -> s64 {
      return x;
    }

    fun f(x: s64) -> s64 {
      return x;
    }

    fun main() {
      f(g(42));
    }
  ";

  let (semantic, report) = analyze_and_validate(source);

  let const_int = semantic.sir.instructions.iter().find_map(|insn| {
    if let Insn::ConstInt { ty_id, .. } = insn {
      Some(*ty_id)
    } else {
      None
    }
  });

  assert!(
    const_int.is_some(),
    "expected a ConstInt in SIR for the literal 42",
  );
  assert_eq!(
    const_int.unwrap().0,
    9,
    "expected ConstInt.ty_id == s64 (TyId 9); \
     the literal should adopt g's param type",
  );

  assert!(
    report.is_ok(),
    "validator should accept Phase 2's clean SIR; got: {:#?}",
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
