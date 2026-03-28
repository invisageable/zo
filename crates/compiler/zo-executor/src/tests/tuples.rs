use crate::tests::common::{assert_no_errors, assert_sir_structure};

use zo_sir::Insn;

#[test]
fn test_tuple_literal_emits_insn() {
  assert_sir_structure(
    r#"fun main() {
  imu t := (1, 2);
}"#,
    |sir| {
      let has_tuple =
        sir.iter().any(|i| matches!(i, Insn::TupleLiteral { .. }));

      assert!(has_tuple, "expected TupleLiteral instruction");
    },
  );
}

#[test]
fn test_tuple_three_elements() {
  assert_sir_structure(
    r#"fun main() {
  imu t := (1, 2, 3);
}"#,
    |sir| {
      let tuple = sir.iter().find(|i| matches!(i, Insn::TupleLiteral { .. }));

      if let Some(Insn::TupleLiteral { elements, .. }) = tuple {
        assert_eq!(
          elements.len(),
          3,
          "expected 3 elements, got {}",
          elements.len()
        );
      } else {
        panic!("expected TupleLiteral instruction");
      }
    },
  );
}

#[test]
fn test_tuple_index_emits_insn() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu t := (10, 20);
  t.0
}"#,
    |sir| {
      let has_index = sir.iter().any(|i| matches!(i, Insn::TupleIndex { .. }));

      assert!(has_index, "expected TupleIndex instruction");
    },
  );
}

#[test]
fn test_tuple_index_correct_field() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu t := (10, 20);
  t.1
}"#,
    |sir| {
      let index_insn =
        sir.iter().find(|i| matches!(i, Insn::TupleIndex { .. }));

      if let Some(Insn::TupleIndex { index, .. }) = index_insn {
        assert_eq!(*index, 1, "expected index 1, got {index}");
      } else {
        panic!("expected TupleIndex instruction");
      }
    },
  );
}

#[test]
fn test_grouping_not_tuple() {
  assert_sir_structure(
    r#"fun main() -> int {
  (42)
}"#,
    |sir| {
      let has_tuple =
        sir.iter().any(|i| matches!(i, Insn::TupleLiteral { .. }));

      assert!(!has_tuple, "grouping (42) should NOT produce TupleLiteral");
    },
  );
}

#[test]
fn test_tuple_type_annotation() {
  assert_sir_structure(
    r#"fun main() {
  imu t: (int, int) = (1, 2);
}"#,
    |sir| {
      let has_tuple =
        sir.iter().any(|i| matches!(i, Insn::TupleLiteral { .. }));

      assert!(
        has_tuple,
        "expected TupleLiteral for typed tuple declaration"
      );
    },
  );
}

#[test]
fn test_tuple_typed_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu t: (int, int) = (3, 7);
}"#,
  );
}

#[test]
fn test_tuple_mixed_typed_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu hero: (str, int, int) = ("johndoe", 100, 15);
}"#,
  );
}

#[test]
fn test_tuple_element_arithmetic() {
  // t.0 + t.1 should produce a BinOp with int type.
  assert_sir_structure(
    r#"fun main() -> int {
  imu t := (10, 20);
  t.0 + t.1
}"#,
    |sir| {
      let has_binop = sir.iter().any(|i| matches!(i, Insn::BinOp { .. }));

      assert!(has_binop, "expected BinOp for t.0 + t.1");
    },
  );
}
