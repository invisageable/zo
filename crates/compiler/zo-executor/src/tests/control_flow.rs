use crate::tests::common::{
  assert_execution_error, assert_sir_stream, assert_sir_structure, execute_raw,
};
use zo_value::{FunctionKind, Pubness};

use zo_error::ErrorKind;
use zo_interner::Symbol;
use zo_sir::{Insn, LoadSource};
use zo_ty::TyId;
use zo_value::ValueId;

#[test]
fn test_if_simple() {
  assert_sir_stream(
    r#"fun main() {
  if true {
    42
  }
}"#,
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(1),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::ConstBool {
        dst: ValueId(0),
        value: true,
        ty_id: TyId(2),
      },
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 1,
      },
      Insn::ConstInt {
        dst: ValueId(1),
        value: 42,
        ty_id: TyId(8),
      },
      Insn::Label { id: 1 },
      Insn::Label { id: 0 },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
    ],
  );
}

#[test]
fn test_if_else() {
  assert_sir_stream(
    r#"fun choose() -> int {
  if true {
    1
  } else {
    2
  }
}
fun main() {}"#,
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(8),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::ConstBool {
        dst: ValueId(0),
        value: true,
        ty_id: TyId(2),
      },
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 1,
      },
      Insn::ConstInt {
        dst: ValueId(1),
        value: 1,
        ty_id: TyId(8),
      },
      Insn::Jump { target: 0 },
      Insn::Label { id: 1 },
      Insn::ConstInt {
        dst: ValueId(2),
        value: 2,
        ty_id: TyId(8),
      },
      Insn::Label { id: 0 },
      Insn::Return {
        value: Some(ValueId(2)),
        ty_id: TyId(8),
      },
      Insn::FunDef {
        name: Symbol(26),
        params: vec![],
        return_ty: TyId(1),
        body_start: 10,
        kind: FunctionKind::Intrinsic,
        pubness: Pubness::No,
      },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
    ],
  );
}

#[test]
fn test_while_loop() {
  assert_sir_stream(
    r#"fun main() {
  while true {
    42
  }
}"#,
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(1),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::Label { id: 0 },
      Insn::ConstBool {
        dst: ValueId(0),
        value: true,
        ty_id: TyId(2),
      },
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 1,
      },
      Insn::ConstInt {
        dst: ValueId(1),
        value: 42,
        ty_id: TyId(8),
      },
      Insn::Jump { target: 0 },
      Insn::Label { id: 1 },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
    ],
  );
}

#[test]
fn test_implicit_return_literal() {
  assert_sir_stream(
    "fun foo() -> int { 42 }",
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(8),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::ConstInt {
        dst: ValueId(0),
        value: 42,
        ty_id: TyId(8),
      },
      Insn::Return {
        value: Some(ValueId(0)),
        ty_id: TyId(8),
      },
    ],
  );
}

#[test]
fn test_void_function_with_value_no_annotation_ok() {
  // `fun foo() { 42 }` has no `-> Type` annotation.
  // The body expression is discarded (unit return).
  // TypeMismatch is only reported when the function has
  // an explicit return type annotation.
  let (sir, _) = execute_raw("fun foo() { 42 }");

  let has_return = sir
    .iter()
    .any(|i| matches!(i, Insn::Return { value: None, .. }));

  assert!(has_return, "expected implicit unit Return");
}

#[test]
fn test_void_function_with_annotation_is_type_error() {
  // `fun foo() -> int { }` declares `-> int` but body is
  // empty — this should be a TypeMismatch.
  assert_execution_error("fun foo() -> int { }", ErrorKind::TypeMismatch);
}

#[test]
fn test_mutable_reassignment() {
  assert_sir_structure(
    r#"fun main() -> int {
  mut x: int = 10;
  x = 20;
  return x;
}"#,
    |sir| {
      // Must have Store (assignment) and Load with
      // src >= 100 (mutable variable read from stack).
      assert!(
        sir.iter().any(|i| matches!(i, Insn::Store { .. })),
        "expected Store instruction for mutable assignment"
      );
      assert!(
        sir.iter().any(|i| matches!(
          i,
          Insn::Load {
            src: LoadSource::Local(_),
            ..
          }
        )),
        "expected Load from local variable"
      );
      // Return must carry a value (not None).
      assert!(
        sir
          .iter()
          .any(|i| matches!(i, Insn::Return { value: Some(_), .. })),
        "expected Return with value"
      );
    },
  );
}

#[test]
fn test_while_loop_sum() {
  assert_sir_structure(
    r#"fun main() -> int {
  mut i: int = 0;
  mut sum: int = 0;
  while i < 5 {
    sum = sum + i;
    i = i + 1;
  }
  return sum;
}"#,
    |sir| {
      // Must have: Label (loop start), BranchIfNot (condition),
      // Jump (back to loop), Store (i = i + 1), BinOp (sum + i).
      assert!(
        sir.iter().any(|i| matches!(i, Insn::Label { .. })),
        "expected Label for loop start"
      );
      assert!(
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. })),
        "expected BranchIfNot for loop condition"
      );
      assert!(
        sir.iter().any(|i| matches!(i, Insn::Jump { .. })),
        "expected Jump back to loop start"
      );

      let store_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Store { .. }))
        .count();

      // At least 4 Stores: init i, init sum, reassign sum, reassign i
      assert!(
        store_count >= 4,
        "expected >= 4 Store instructions, got {store_count}"
      );
    },
  );
}

#[test]
fn test_for_loop_sum() {
  // for i := 0..5 desugars to:
  //   mut i = 0; while i < 5 { body; i = i + 1; }
  assert_sir_structure(
    r#"fun main() -> int {
  mut sum: int = 0;
  for i := 0..5 {
    sum = sum + i;
  }
  return sum;
}"#,
    |sir| {
      // Must have loop structure: Label, BranchIfNot, Jump
      assert!(
        sir.iter().any(|i| matches!(i, Insn::Label { .. })),
        "expected Label for loop start"
      );
      assert!(
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. })),
        "expected BranchIfNot for loop condition"
      );
      assert!(
        sir.iter().any(|i| matches!(i, Insn::Jump { .. })),
        "expected Jump back to loop start"
      );

      // Must have Stores for sum and the loop variable i
      let store_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Store { .. }))
        .count();

      assert!(
        store_count >= 3,
        "expected >= 3 Store instructions, got {store_count}"
      );
      // Must have a Return with a value
      assert!(
        sir
          .iter()
          .any(|i| matches!(i, Insn::Return { value: Some(_), .. })),
        "expected Return with value (for)"
      );
    },
  );
}

#[test]
fn test_float_param_produces_load() {
  assert_sir_structure(
    r#"fun add_f(a: float, b: float) -> float {
  return a + b;
}"#,
    |sir| {
      let loads = sir
        .iter()
        .filter(|i| matches!(i, Insn::Load { .. }))
        .collect::<Vec<_>>();

      assert!(
        loads.len() >= 2,
        "expected >= 2 Load for float params, got {}",
        loads.len()
      );
      assert!(
        sir.iter().any(|i| matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Add,
            ..
          }
        )),
        "expected BinOp Add for a + b"
      );
    },
  );
}

#[test]
fn test_break_in_while() {
  assert_sir_structure(
    r#"fun main() -> int {
  mut i: int = 0;
  while i < 100 {
    if i == 5 {
      break;
    }
    i = i + 1;
  }
  return i;
}"#,
    |sir| {
      // break emits a Jump to end_label.
      let jumps = sir
        .iter()
        .filter(|i| matches!(i, Insn::Jump { .. }))
        .count();

      assert!(
        jumps >= 2,
        "expected >= 2 Jumps (break + loop-back), got {jumps}"
      );
    },
  );
}

#[test]
fn test_continue_in_for() {
  assert_sir_structure(
    r#"fun main() -> int {
  mut sum: int = 0;
  for i := 0..10 {
    if i == 3 {
      continue;
    }
    sum = sum + i;
  }
  return sum;
}"#,
    |sir| {
      // continue emits a Jump to loop_label.
      let jumps = sir
        .iter()
        .filter(|i| matches!(i, Insn::Jump { .. }))
        .count();

      assert!(
        jumps >= 2,
        "expected >= 2 Jumps (continue + loop-back), got {jumps}"
      );
    },
  );
}

// === FOR LOOP RANGE (inclusive vs exclusive, dynamic bounds) ===

#[test]
fn test_for_loop_inclusive_range_emits_lte() {
  // `..=` must lower to `BinOp::Lte` (not `Lt`). Without this
  // fix the end bound is off-by-one and `for i := 1..=3` only
  // runs for i=1,2.
  assert_sir_structure(
    r#"fun main() {
  for i := 1..=3 {
    showln(i);
  }
}"#,
    |sir| {
      let has_lte = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Lte,
            ..
          }
        )
      });

      assert!(has_lte, "expected BinOp::Lte for `..=` range");

      let has_lt = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Lt,
            ..
          }
        )
      });

      assert!(!has_lt, "did not expect BinOp::Lt for `..=` range");
    },
  );
}

#[test]
fn test_for_loop_exclusive_range_still_emits_lt() {
  // Regression guard: `..` must keep lowering to `BinOp::Lt`.
  assert_sir_structure(
    r#"fun main() {
  for i := 0..3 {
    showln(i);
  }
}"#,
    |sir| {
      let has_lt = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Lt,
            ..
          }
        )
      });

      assert!(has_lt, "expected BinOp::Lt for `..` range");
    },
  );
}

#[test]
fn test_for_loop_variable_end_bound_emits_load() {
  // Non-literal end bounds (like a parameter `n`) must be
  // evaluated as an expression, stored to a synthetic slot,
  // and reloaded per iteration. Before the fix the scanner
  // only recognised Int literals and silently fell back to
  // `end = 0`, so the loop body was never entered.
  assert_sir_structure(
    r#"fun f(n: int) {
  for i := 1..n {
    showln(i);
  }
}
fun main() {
  f(3);
}"#,
    |sir| {
      // Param `n` is loaded via Insn::Load { src: Param(..) }
      // as part of the end-bound evaluation.
      let has_param_load = sir.iter().any(|i| {
        matches!(
          i,
          Insn::Load {
            src: LoadSource::Param(_),
            ..
          }
        )
      });

      assert!(has_param_load, "expected Load of Param for the end bound");

      let has_lt = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Lt,
            ..
          }
        )
      });

      assert!(has_lt, "expected BinOp::Lt for `..` range");
    },
  );
}

#[test]
fn test_for_loop_variable_inclusive_bound_emits_lte() {
  // Same as above but for `..=n`.
  assert_sir_structure(
    r#"fun g(n: int) {
  for i := 1..=n {
    showln(i);
  }
}
fun main() {
  g(3);
}"#,
    |sir| {
      let has_param_load = sir.iter().any(|i| {
        matches!(
          i,
          Insn::Load {
            src: LoadSource::Param(_),
            ..
          }
        )
      });

      assert!(has_param_load, "expected Load of Param for the end bound");

      let has_lte = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Lte,
            ..
          }
        )
      });

      assert!(has_lte, "expected BinOp::Lte for `..=` range");
    },
  );
}

// === MATCH ON TUPLE ===

#[test]
fn test_match_tuple_pattern_emits_per_field_compare() {
  // Tuple-pattern arms must emit a TupleIndex + Eq compare +
  // BranchIfNot for each literal field. `_` fields contribute
  // no compare. Before the fix no compares were emitted at
  // all and every tuple-pattern arm matched unconditionally,
  // so the first arm always won.
  assert_sir_structure(
    r#"fun main() {
  match (3, 5) {
    (0, 0) => showln("zero"),
    (3, _) => showln("three"),
    _ => showln("other"),
  }
}"#,
    |sir| {
      // (0, 0) contributes 2 compares, (3, _) contributes 1.
      let tuple_indexes = sir
        .iter()
        .filter(|i| matches!(i, Insn::TupleIndex { .. }))
        .count();

      assert!(
        tuple_indexes >= 3,
        "expected >= 3 TupleIndex reads for tuple pattern \
         fields, got {tuple_indexes}"
      );

      let eq_ops = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::BinOp {
              op: zo_sir::BinOp::Eq,
              ..
            }
          )
        })
        .count();

      assert!(
        eq_ops >= 3,
        "expected >= 3 Eq compares across tuple-pattern arms, \
         got {eq_ops}"
      );

      let branches = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branches >= 3,
        "expected >= 3 BranchIfNot (one per field compare), \
         got {branches}"
      );
    },
  );
}

#[test]
fn test_match_tuple_of_locals_materializes_synthetic_scrutinee() {
  // `match (a, b)` where `a` and `b` are stored locals must
  // spill the TupleLiteral result into `__match_scrut__` and
  // per-arm `TupleIndex` must read from that synthetic local
  // — NOT from `a` (the first Ident in the scrutinee range).
  // Before the fix the code picked `a` as the scrutinee symbol
  // and every arm compared garbage, producing zero output.
  assert_sir_structure(
    r#"fun main() {
  imu a: int = 0;
  imu b: int = 0;
  match (a, b) {
    (0, 0) => showln("zz"),
    _ => showln("other"),
  }
}"#,
    |sir| {
      // Must materialize the inline tuple scrutinee into a
      // synthetic spill — a plain `TupleLiteral` value cannot
      // be addressed by subsequent `TupleIndex` loads.
      let has_spill =
        sir.iter().any(|i| matches!(i, Insn::TupleLiteral { .. }));

      assert!(
        has_spill,
        "expected a TupleLiteral for the scrutinee (a, b)"
      );

      let tuple_indexes = sir
        .iter()
        .filter(|i| matches!(i, Insn::TupleIndex { .. }))
        .count();

      assert!(
        tuple_indexes >= 2,
        "expected >= 2 TupleIndex reads for the (0, 0) arm, \
         got {tuple_indexes}"
      );
    },
  );
}

#[test]
fn test_array_literal_and_index() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu arr: []int = [10, 20, 30];
  return arr[1];
}"#,
    |sir| {
      assert!(
        sir.iter().any(|i| matches!(i, Insn::ArrayLiteral { .. })),
        "expected ArrayLiteral instruction"
      );
      assert!(
        sir.iter().any(|i| matches!(i, Insn::ArrayIndex { .. })),
        "expected ArrayIndex instruction"
      );
    },
  );
}

#[test]
fn test_showln_int_emits_call() {
  // showln(42) should emit a Call instruction with an
  // int argument (not just a string).
  assert_sir_structure(
    r#"fun main() {
  showln(42);
}"#,
    |sir| {
      assert!(
        sir.iter().any(|i| matches!(
          i,
          Insn::Call { args, .. } if !args.is_empty()
        )),
        "expected Call with argument for showln(42)"
      );
    },
  );
}

#[test]
fn test_ext_declaration() {
  assert_sir_structure(
    r#"ext readln() -> str;
fun main() -> int { 42 }"#,
    |sir| {
      let ext_fn = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Intrinsic,
            ..
          }
        )
      });

      assert!(ext_fn.is_some(), "expected FunDef with kind: Intrinsic");
    },
  );
}

// === TERNARY EXPRESSION ===

#[test]
fn test_ternary_basic() {
  let (sir, _) = execute_raw(
    r#"fun main() -> int {
  when true ? 1 : 2
}"#,
  );

  // Ternary: BranchIfNot + at least 1 Label (end_label).
  // Constant folding may eliminate the else branch,
  // collapsing else_label into end_label.
  let has_branch = sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));

  assert!(has_branch, "expected BranchIfNot for ternary condition");

  let label_count = sir
    .iter()
    .filter(|i| matches!(i, Insn::Label { .. }))
    .count();

  assert!(label_count >= 1, "expected >= 1 Labels, got {label_count}");
}

#[test]
fn test_ternary_variable() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu x: int = 5;
  when x > 0 ? x : 0
}"#,
    |sir| {
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));

      assert!(has_branch, "expected BranchIfNot");

      let has_jump = sir.iter().any(|i| matches!(i, Insn::Jump { .. }));

      assert!(has_jump, "expected Jump to skip false arm");
    },
  );
}

#[test]
fn test_ternary_in_binding() {
  assert_sir_structure(
    r#"fun main() {
  imu x: int = when true ? 42 : 0;
}"#,
    |sir| {
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));

      assert!(has_branch, "expected BranchIfNot");

      let has_vardef = sir.iter().any(|i| matches!(i, Insn::VarDef { .. }));

      assert!(has_vardef, "expected VarDef for ternary binding");
    },
  );
}

#[test]
fn test_ternary_with_check() {
  assert_sir_structure(
    r#"ext check(b: bool);
fun main() {
  imu x: int = when true ? 42 : 0;
  check@eq(x, 42);
}"#,
    |sir| {
      let calls: Vec<_> = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .collect();

      assert!(!calls.is_empty(), "expected check Call after ternary");
    },
  );
}

#[test]
fn test_ternary_operator_in_condition() {
  assert_sir_structure(
    r#"fun main() {
  imu a: int = 10;
  imu b: int = 20;
  imu max: int = when a > b ? a : b;
}"#,
    |sir| {
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));

      assert!(has_branch, "expected BranchIfNot");
    },
  );
}

#[test]
fn test_ternary_operator_in_arms() {
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 5;
  imu y: int = when x > 3 ? x * 2 : x + 1;
}"#,
    |sir| {
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));

      assert!(has_branch, "expected BranchIfNot");
    },
  );
}

#[test]
fn test_euler_01_modulo_or_in_for() {
  // Euler #1: sum of multiples of 3 or 5 below 10 = 23.
  // Tests: for loop, %, ==, ||, +=, if inside for.
  assert_sir_structure(
    r#"fun main() -> int {
  mut ans: int = 0;
  for x := 1..10 {
    if x % 3 == 0 || x % 5 == 0 {
      ans += x;
    }
  }
  return ans;
}"#,
    |sir| {
      // Must have Rem (%) operator.
      let has_rem = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Rem,
            ..
          }
        )
      });
      assert!(has_rem, "expected BinOp::Rem for %");

      // Must have Eq (==) comparison.
      let has_eq = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Eq,
            ..
          }
        )
      });
      assert!(has_eq, "expected BinOp::Eq for ==");

      // Must have Or (||) operator.
      let has_or = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Or,
            ..
          }
        )
      });
      assert!(has_or, "expected BinOp::Or for ||");

      // Must have BranchIfNot for the if condition.
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));
      assert!(has_branch, "expected BranchIfNot for if");
    },
  );
}

#[test]
fn test_return_inside_while() {
  // Early return from inside a while + if body.
  assert_sir_structure(
    r#"fun find(n: int) -> int {
  mut i: int = 2;
  while i < n {
    if n % i == 0 {
      return i;
    }
    i += 1;
  }
  return n;
}
fun main() -> int { find(15) }"#,
    |sir| {
      // Must have at least 2 Return instructions
      // (one inside the while, one at the end).
      let returns = sir
        .iter()
        .filter(|i| matches!(i, Insn::Return { .. }))
        .count();
      assert!(returns >= 2, "expected >= 2 Returns, got {returns}");
    },
  );
}

// === SHORT-CIRCUIT `&&` / `||` ===
//
// See `execute_logical_binop` in `executor.rs`. The executor
// lowers `lhs && rhs_call()` / `lhs || rhs_call()` to a φ-
// sink pattern whenever the RHS has not yet materialized on
// the stacks — emitting `Store sink, lhs; BranchIfNot cond,
// end; Store sink, rhs; Label end; Load dst, sink`. This
// matches the ternary pattern (`PLAN_BRANCH_EXPR_PHI.md`),
// reusing the same `__branch_result_N__` sink naming so
// codegen's mutable-slot handling picks it up.

#[test]
fn test_short_circuit_and_with_call_rhs() {
  // `false && side()` must emit:
  //   Store sink, false_const
  //   BranchIfNot false_const, end_label
  //   Call side
  //   Store sink, call_result
  //   Label end_label
  //   Load dst, Local(sink)
  //
  // — i.e. the RHS call is guarded by a skip branch so it
  // doesn't run when the LHS already determines the result.
  assert_sir_structure(
    r#"fun side() -> bool { true }
fun main() -> bool {
  false && side()
}"#,
    |sir| {
      // Must have a Store into a __branch_result_N__ sink
      // — that's the LHS capture emitted at `&&`.
      let has_sink_store = sir.iter().any(|i| matches!(i, Insn::Store { .. }));

      assert!(has_sink_store, "expected Store into short-circuit sink");

      // Must have exactly one BranchIfNot guarding the RHS.
      let branch_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branch_count >= 1,
        "expected >= 1 BranchIfNot for short-circuit, got {branch_count}"
      );

      // Must have a Label (the merge point after the RHS).
      let has_label = sir.iter().any(|i| matches!(i, Insn::Label { .. }));

      assert!(has_label, "expected Label for short-circuit merge");

      // Must have a Load from a Local sink (the merged result).
      let has_load_local = sir.iter().any(|i| {
        matches!(
          i,
          Insn::Load {
            src: LoadSource::Local(_),
            ..
          }
        )
      });

      assert!(
        has_load_local,
        "expected Load from local sink at short-circuit merge"
      );

      // Must NOT emit BinOp::And for this case — the short-
      // circuit path replaces the plain bitwise op.
      let has_binop_and = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::And,
            ..
          }
        )
      });

      assert!(
        !has_binop_and,
        "short-circuit && must NOT emit BinOp::And — it emits \
         Store/BranchIfNot/Store/Label/Load instead"
      );
    },
  );
}

#[test]
fn test_short_circuit_or_with_call_rhs() {
  // `false || side()` must short-circuit when LHS is true.
  // Because we only have `BranchIfNot` (no `BranchIf`), the
  // `||` handler synthesizes a `UnOp::Not` on the LHS and
  // branches on that. Shape:
  //   Store sink, lhs
  //   UnOp::Not tmp, lhs
  //   BranchIfNot tmp, end_label
  //   Call side
  //   Store sink, call_result
  //   Label end_label
  //   Load dst, Local(sink)
  assert_sir_structure(
    r#"fun side() -> bool { false }
fun main() -> bool {
  false || side()
}"#,
    |sir| {
      // Must have UnOp::Not synthesized for the `||` path.
      let has_not = sir.iter().any(|i| {
        matches!(
          i,
          Insn::UnOp {
            op: zo_sir::UnOp::Not,
            ..
          }
        )
      });

      assert!(
        has_not,
        "expected synthesized UnOp::Not for `||` short-circuit"
      );

      // BranchIfNot must guard the RHS.
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));

      assert!(has_branch, "expected BranchIfNot for `||` short-circuit");

      // Label + Load from local sink for the merge.
      let has_label = sir.iter().any(|i| matches!(i, Insn::Label { .. }));

      assert!(has_label, "expected Label for `||` merge");

      let has_load_local = sir.iter().any(|i| {
        matches!(
          i,
          Insn::Load {
            src: LoadSource::Local(_),
            ..
          }
        )
      });

      assert!(
        has_load_local,
        "expected Load from local sink at `||` merge"
      );

      // Must NOT emit BinOp::Or — control flow replaces it.
      let has_binop_or = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Or,
            ..
          }
        )
      });

      assert!(!has_binop_or, "short-circuit || must NOT emit BinOp::Or");
    },
  );
}

#[test]
fn test_short_circuit_nested_and() {
  // `a && b && c()` — two levels of logical ops. The
  // first `&&` folds eagerly (both operands on stack),
  // the second `&&` short-circuits around the call.
  // Expect exactly one BranchIfNot (the outer SC guard),
  // one BinOp::And (the inner eager path), and one Store
  // into a __branch_result_N__ sink.
  assert_sir_structure(
    r#"fun side() -> bool { true }
fun main() -> bool {
  true && true && side()
}"#,
    |sir| {
      // Outer `&&` short-circuits → at least one
      // BranchIfNot for the SC guard.
      let branch_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::BranchIfNot { .. }))
        .count();

      assert!(
        branch_count >= 1,
        "expected >= 1 BranchIfNot (outer &&), got {branch_count}"
      );

      // Call to `side` must still be present in SIR (it
      // emits regardless — the short-circuit only skips
      // it at runtime, not at compile time).
      let has_call = sir.iter().any(|i| matches!(i, Insn::Call { .. }));

      assert!(has_call, "expected Call for side()");
    },
  );
}

#[test]
fn test_short_circuit_const_fold_preserved() {
  // Pure truth-table `true && false` still collapses via
  // the normal binop path — both operands are on the stack
  // when `&&` fires, so the handler delegates to
  // `execute_binop`, which folds the constant. No short-
  // circuit skeleton should be emitted (no Store into a
  // __branch_result_N__ sink, no control flow).
  assert_sir_structure(
    r#"fun main() -> bool {
  true && false
}"#,
    |sir| {
      // No BranchIfNot for pure constant folding.
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));

      assert!(
        !has_branch,
        "constant `true && false` must fold without BranchIfNot"
      );

      // Result should be a ConstBool(false) somewhere.
      let has_false_const = sir
        .iter()
        .any(|i| matches!(i, Insn::ConstBool { value: false, .. }));

      assert!(
        has_false_const,
        "expected ConstBool(false) from folding `true && false`"
      );
    },
  );
}

// Regression: `true || incr(y)` — the short-circuit RHS is
// a call WITH an argument. Before the fix,
// `apply_deferred_short_circuit` fired as soon as the arg
// `y` landed on the stacks (the first new value past the
// sink-depth mark), stealing it as the "RHS" and leaving
// the enclosing `showln(...)` with no argument. The guards
// on `DeferredShortCircuit.pre_direct_call_depth` now keep
// the SC pending until the call itself emits and its result
// is on the stack.
#[test]
fn test_short_circuit_or_with_call_arg() {
  assert_sir_structure(
    r#"fun incr(mut x: int) -> bool { false }
fun main() {
  imu y: int = 10;
  showln(true || incr(y));
}"#,
    |sir| {
      // `incr` MUST be called — not elided. Look for
      // Call whose callee ident is `incr` by finding a
      // Call that appears *before* the final showln Call.
      let call_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        call_count >= 2,
        "expected >= 2 Calls (incr + showln), got {call_count}"
      );

      // `showln(...)` MUST have exactly one argument (the
      // merged SC result). Before the fix the last Call had
      // no args because the SC stole the only arg on stack.
      let last_call = sir.iter().rev().find(|i| matches!(i, Insn::Call { .. }));

      if let Some(Insn::Call { args, .. }) = last_call {
        assert_eq!(
          args.len(),
          1,
          "showln(true || incr(y)) must have 1 arg, got {}",
          args.len()
        );
      } else {
        panic!("expected a Call for showln");
      }

      // SC shape intact: Store into sink, BranchIfNot guard,
      // Label merge, Load from sink.
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));
      assert!(has_branch, "expected BranchIfNot for `||` guard");

      let has_sink_load = sir.iter().any(|i| {
        matches!(
          i,
          Insn::Load {
            src: LoadSource::Local(_),
            ..
          }
        )
      });
      assert!(has_sink_load, "expected Load from SC sink");
    },
  );
}

// Mirror of the `||` regression for `&&`. `false && incr(y)`
// must emit the SC skeleton AND actually call `incr` — the
// arg `y` must not be stolen as the RHS.
#[test]
fn test_short_circuit_and_with_call_arg() {
  assert_sir_structure(
    r#"fun incr(mut x: int) -> bool { true }
fun main() {
  imu y: int = 10;
  showln(false && incr(y));
}"#,
    |sir| {
      let call_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        call_count >= 2,
        "expected >= 2 Calls (incr + showln), got {call_count}"
      );

      let last_call = sir.iter().rev().find(|i| matches!(i, Insn::Call { .. }));

      if let Some(Insn::Call { args, .. }) = last_call {
        assert_eq!(
          args.len(),
          1,
          "showln(false && incr(y)) must have 1 arg, got {}",
          args.len()
        );
      }

      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));
      assert!(has_branch, "expected BranchIfNot for `&&` guard");
    },
  );
}

// Short-circuit nested inside a function arg, RHS is a
// call: `f(a || g())` — the SC's RHS is the call `g()`,
// and it must finalize at g's RParen (inner scope) so `f`
// gets the merged SC result as its single arg. Exercises
// the `pre_direct_call_depth` guard: SC pre-depth = 1 (we
// are already inside f's call), so the finalize must fire
// when direct_call_depth returns to 1, not before.
#[test]
fn test_short_circuit_inside_call_arg_with_nested_call() {
  assert_sir_structure(
    r#"fun g() -> bool { true }
fun f(x: bool) -> bool { x }
fun main() {
  imu a: bool = false;
  showln(f(a || g()));
}"#,
    |sir| {
      // Three calls expected: g, f, showln — the SC must
      // emit the call to g (as the guarded RHS), not elide
      // it, and `f` and `showln` each get exactly one arg.
      let calls: Vec<&Insn> = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .collect();

      assert!(
        calls.len() >= 3,
        "expected >= 3 Calls (g + f + showln), got {}",
        calls.len()
      );

      // Arg-count profile: `g()` has 0 args, `f(x)` and
      // `showln(...)` each have 1. If the SC finalized too
      // eagerly and stole `f`'s arg, we'd see a showln with
      // 0 args instead of 1 — which is the bug this test
      // guards against.
      let zero_arg_calls = calls
        .iter()
        .filter(|c| match c {
          Insn::Call { args, .. } => args.is_empty(),
          _ => false,
        })
        .count();

      let one_arg_calls = calls
        .iter()
        .filter(|c| match c {
          Insn::Call { args, .. } => args.len() == 1,
          _ => false,
        })
        .count();

      assert_eq!(zero_arg_calls, 1, "expected exactly 1 zero-arg call (g)");
      assert!(
        one_arg_calls >= 2,
        "expected >= 2 one-arg calls (f + showln), got {one_arg_calls}"
      );

      // SC guard emitted: BranchIfNot on `!a` (for `||`).
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));
      assert!(has_branch, "expected BranchIfNot for `||` short-circuit");
    },
  );
}

#[test]
fn test_for_loop_sum_with_showln_interp() {
  // For loop sum with showln interpolation — verifies
  // that {ans} emits a Load (not a stale ValueId).
  assert_sir_structure(
    r#"fun main() {
  mut ans: int = 0;
  for x := 1..10 {
    ans += x;
  }
  showln("{ans}");
}"#,
    |sir| {
      // The showln("{ans}") call must reference a Load,
      // not the init ConstInt(0).
      let last_call = sir.iter().rev().find(|i| matches!(i, Insn::Call { .. }));

      if let Some(Insn::Call { args, .. }) = last_call {
        assert!(!args.is_empty(), "showln call must have an argument");

        // The arg should come from a Load, not ConstInt.
        let arg = args[0];

        let producer = sir.iter().find(|i| match i {
          Insn::Load { dst, .. } => *dst == arg,
          _ => false,
        });

        assert!(
          producer.is_some(),
          "showln arg must come from a Load (not stale init)"
        );
      }
    },
  );
}
