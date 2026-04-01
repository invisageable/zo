use crate::tests::common::{assert_no_errors, execute_raw};

use zo_sir::Insn;

#[test]
fn test_str_array_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu a: []str = ["hello", "world"];
}"#,
  );
}

#[test]
fn test_bool_array_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu a: []bool = [true, false];
}"#,
  );
}

#[test]
fn test_float_array_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu a: []float = [3.14, 2.71];
}"#,
  );
}

#[test]
fn test_array_literal_emits_correct_element_count() {
  let (insns, _) = execute_raw(
    r#"fun main() {
  imu a: []int = [1, 2, 3];
}"#,
  );

  let array_lit = insns
    .iter()
    .find(|i| matches!(i, Insn::ArrayLiteral { .. }));

  match array_lit {
    Some(Insn::ArrayLiteral { elements, .. }) => {
      assert_eq!(
        elements.len(),
        3,
        "expected 3 elements, got {}",
        elements.len()
      );
    }
    _ => panic!("expected ArrayLiteral in SIR"),
  }
}

#[test]
fn test_static_array_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu a: [3]int = [10, 20, 30];
}"#,
  );
}

#[test]
fn test_2d_array_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu r0: []int = [1, 2];
  imu r1: []int = [3, 4];
  imu grid: [][]int = [r0, r1];
}"#,
  );
}

#[test]
fn test_chained_index_emits_two_array_index() {
  let (insns, _) = execute_raw(
    r#"fun main() {
  imu r0: []int = [1, 2];
  imu r1: []int = [3, 4];
  imu grid: [][]int = [r0, r1];
  imu v: int = grid[0][0];
}"#,
  );

  let count = insns
    .iter()
    .filter(|i| matches!(i, Insn::ArrayIndex { .. }))
    .count();

  assert_eq!(
    count, 2,
    "expected 2 ArrayIndex for grid[0][0], got {count}"
  );
}

#[test]
fn test_2d_static_type_annotation_no_errors() {
  assert_no_errors(
    r#"fun main() {
  imu g: [2][3]int = [[1, 2, 3], [4, 5, 6]];
}"#,
  );
}
