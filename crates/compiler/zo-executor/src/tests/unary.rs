use crate::tests::common::assert_sir_structure;

use zo_sir::{Insn, UnOp};

// === UNARY NEGATION ===

#[test]
fn test_neg_literal_folds_to_const() {
  assert_sir_structure(
    r#"fun main() {
  imu x: int = -42;
}"#,
    |sir| {
      // -42 should constant-fold to ConstInt(-42).
      let has_neg_const = sir.iter().any(
        |i| matches!(i, Insn::ConstInt { value, .. } if *value as i64 == -42),
      );

      assert!(
        has_neg_const,
        "-42 should fold to ConstInt(-42), got: {sir:#?}"
      );
    },
  );
}

#[test]
fn test_neg_variable_emits_unop() {
  assert_sir_structure(r#"fun negate(n: int) -> int { return -n }"#, |sir| {
    let has_neg = sir
      .iter()
      .any(|i| matches!(i, Insn::UnOp { op: UnOp::Neg, .. }));

    assert!(has_neg, "-n should emit UnOp::Neg, got: {sir:#?}");
  });
}

#[test]
fn test_neg_in_return_emits_unop() {
  assert_sir_structure(
    r#"fun f(x: int) -> int {
  if x < 0 {
    return -x;
  }
  return x;
}"#,
    |sir| {
      let has_neg = sir
        .iter()
        .any(|i| matches!(i, Insn::UnOp { op: UnOp::Neg, .. }));

      assert!(has_neg, "return -x should emit UnOp::Neg, got: {sir:#?}");
    },
  );
}

// === UNARY NOT ===

#[test]
fn test_not_literal_folds_to_const() {
  assert_sir_structure(
    r#"fun main() {
  imu x: bool = !true;
}"#,
    |sir| {
      let has_false = sir
        .iter()
        .any(|i| matches!(i, Insn::ConstBool { value: false, .. }));

      assert!(
        has_false,
        "!true should fold to ConstBool(false), got: {sir:#?}"
      );
    },
  );
}

#[test]
fn test_not_variable_emits_unop() {
  assert_sir_structure(r#"fun invert(b: bool) -> bool { return !b }"#, |sir| {
    let has_not = sir
      .iter()
      .any(|i| matches!(i, Insn::UnOp { op: UnOp::Not, .. }));

    assert!(has_not, "!b should emit UnOp::Not, got: {sir:#?}");
  });
}
