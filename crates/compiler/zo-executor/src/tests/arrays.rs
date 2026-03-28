use crate::tests::common::assert_sir_structure;

use zo_sir::Insn;

#[test]
fn test_array_literal_produces_sir() {
  assert_sir_structure(
    r#"fun main() {
  imu x: int[] = [1, 2, 3];
}"#,
    |sir| {
      let has_array =
        sir.iter().any(|i| matches!(i, Insn::ArrayLiteral { .. }));

      assert!(has_array, "expected ArrayLiteral in SIR");
    },
  );
}

#[test]
fn test_array_index_produces_sir() {
  assert_sir_structure(
    r#"fun main() {
  imu x: int[] = [10, 20, 30];
  imu v: int = x[0];
}"#,
    |sir| {
      let has_index = sir.iter().any(|i| matches!(i, Insn::ArrayIndex { .. }));

      assert!(has_index, "expected ArrayIndex in SIR");
    },
  );
}

#[test]
fn test_array_with_showln() {
  assert_sir_structure(
    r#"fun main() {
  imu x: int[] = [10, 25, 50];
  imu v: int = x[0];
  showln(v);
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        calls >= 1,
        "expected at least 1 Call for showln, got {}",
        calls
      );
    },
  );
}

#[test]
fn test_array_binop_two_indices() {
  // a[0] + a[1] should produce two ArrayIndex then BinOp.
  assert_sir_structure(
    r#"fun main() {
  imu a: int[] = [5, 12, 8];
  imu c: int = a[0] + a[1];
  showln(c);
}"#,
    |sir| {
      let arr_idx_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::ArrayIndex { .. }))
        .count();

      assert_eq!(
        arr_idx_count, 2,
        "expected 2 ArrayIndex, got {}",
        arr_idx_count
      );

      // BinOp should reference both ArrayIndex results.
      let has_binop = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Add,
            ..
          }
        )
      });

      assert!(has_binop, "expected Add BinOp");
    },
  );
}

#[test]
fn test_array_with_interp_showln() {
  // Array + interpolation with prefix text.
  assert_sir_structure(
    r#"fun main() {
  imu x: int[] = [10, 25, 50];
  imu v: int = x[0];
  showln("value: {v}");
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      assert!(
        calls >= 2,
        "expected >= 2 Call instructions for \
         interpolation desugaring, got {}",
        calls
      );
    },
  );
}

#[test]
fn test_interp_with_prefix_no_array() {
  // Interpolation with prefix text: showln("value: {x}")
  // desugars to show("value: ") + showln(x).
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 42;
  showln("value: {x}");
  showln("done");
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .count();

      // show("value: ") + showln(x) + showln("done")
      assert!(calls >= 3, "expected >= 3 Call instructions, got {}", calls);
    },
  );
}
