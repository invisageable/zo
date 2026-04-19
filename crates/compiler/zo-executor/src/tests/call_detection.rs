//! Call-vs-group detection tests — regressions for issue H's
//! secondary fix. The parser now emits `Ident LParen` adjacent
//! for `x + (1)` (where `x` is a plain local variable), so
//! `resolve_call_target` / `ident_is_call_target` must reject
//! the call interpretation when the ident is a non-closure
//! local — otherwise the executor tries to call the variable
//! and the surrounding decl fails.

use crate::tests::common::{assert_no_errors, assert_sir_structure};

use zo_sir::{BinOp, Insn};

#[test]
fn test_group_after_ident_lhs_is_not_a_call() {
  // `x + (1)` — `x` is a plain int local; the adjacent
  // `(` is a grouping paren, not a call. SIR must emit
  // an Add BinOp and NO Call for `x`.
  assert_sir_structure(
    r#"fun main() {
  mut x: int = 3;
  imu a: int = x + (1);
}"#,
    |sir| {
      let add_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::BinOp { op: BinOp::Add, .. }))
        .count();

      assert!(add_count >= 1, "expected at least one Add BinOp");

      let call_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert_eq!(
        call_count, 0,
        "expected no Call insns (x + (1) must not call x)"
      );
    },
  );
}

#[test]
fn test_group_with_while_midpoint_emits_loop() {
  // The binary-search pattern that exposed issue H —
  // assert it emits SIR with Jump / BranchIfNot (loop
  // structure) and an Add BinOp for `low + (...)`.
  assert_sir_structure(
    r#"fun main() {
  mut low: int = 0;
  mut high: int = 4;
  mut iters: int = 0;

  while low <= high {
    imu mid: int = low + (high - low) / 2;
    iters = iters + 1;
    low = mid + 1;
  }
}"#,
    |sir| {
      let has_jump = sir.iter().any(|i| matches!(i, Insn::Jump { .. }));
      let has_branch =
        sir.iter().any(|i| matches!(i, Insn::BranchIfNot { .. }));
      let has_div = sir
        .iter()
        .any(|i| matches!(i, Insn::BinOp { op: BinOp::Div, .. }));

      assert!(has_jump, "expected Jump (loop back-edge)");
      assert!(has_branch, "expected BranchIfNot (loop condition)");
      assert!(has_div, "expected Div BinOp (mid calculation)");
    },
  );
}

#[test]
fn test_user_fun_call_after_operator_emits_call() {
  // `2 + five()` — parser emits `2, five, (, ), +`.
  // Executor's call-detection must STILL recognize
  // `five` as a callee (it's in self.funs).
  assert_sir_structure(
    r#"fun five() -> int {
  return 5;
}

fun main() {
  imu a: int = 2 + five();
}"#,
    |sir| {
      let call_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        call_count >= 1,
        "expected at least 1 Call for five(), got {}",
        call_count
      );

      let add_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::BinOp { op: BinOp::Add, .. }))
        .count();

      assert!(add_count >= 1, "expected at least one Add BinOp");
    },
  );
}

#[test]
fn test_colon_colon_path_preserved_as_call() {
  // `Point::new(1, 2)` — `::` prefix forces the call
  // interpretation; `new` isn't in self.funs by its
  // bare name (stored mangled as `Point::new`).
  assert_sir_structure(
    r#"struct Point {
  x: int,
  y: int,
}

apply Point {
  fun new(x: int, y: int) -> Self {
    Self { x, y }
  }
}

fun main() {
  imu p: Point = Point::new(1, 2);
}"#,
    |sir| {
      let call_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        call_count >= 1,
        "expected at least 1 Call for Point::new, got {}",
        call_count
      );
    },
  );
}

#[test]
fn test_plain_ident_with_trailing_group_no_false_call() {
  // `y + (x - 1)` where both idents are plain locals —
  // neither adjacent `(` triggers a false call.
  assert_no_errors(
    r#"fun main() {
  mut x: int = 5;
  mut y: int = 10;
  imu a: int = y + (x - 1);
}"#,
  );
}
