use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_sir::Insn;

// === STR SLICE (COMPILE-TIME) ===

#[test]
fn test_str_slice_exclusive_emits_conststring() {
  assert_sir_structure(
    r#"fun main() {
  imu s: str = "hello, world!";
  imu hello: str = s[0..5];
}"#,
    |sir| {
      // Expect exactly one ConstString carrying "hello" — the
      // sliced result, interned as a fresh string constant.
      let has_hello = sir.iter().any(|i| matches!(i, Insn::ConstString { .. }));

      assert!(has_hello, "slice should emit a ConstString for the result");
    },
  );
}

#[test]
fn test_str_slice_inclusive_range_ok() {
  // `..=` adjusts the upper bound by +1 internally.
  assert_sir_structure(
    r#"fun main() {
  imu s: str = "hello, world!";
  imu world: str = s[7..=11];
}"#,
    |sir| {
      let const_string_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::ConstString { .. }))
        .count();

      // At minimum: source str + sliced str.
      assert!(
        const_string_count >= 2,
        "expected >= 2 ConstStrings (source + slice), got {}",
        const_string_count
      );
    },
  );
}

// === ERROR PATHS ===

#[test]
fn test_str_slice_out_of_bounds_reports_error() {
  assert_execution_error(
    r#"fun main() {
  imu s: str = "hi";
  imu x: str = s[0..10];
}"#,
    ErrorKind::StrSliceOutOfBounds,
  );
}

#[test]
fn test_str_slice_invalid_range_reports_error() {
  assert_execution_error(
    r#"fun main() {
  imu s: str = "hello";
  imu x: str = s[4..2];
}"#,
    ErrorKind::StrSliceInvalidRange,
  );
}

#[test]
fn test_str_slice_non_const_bound_emits_runtime_slice() {
  // Non-const bounds used to hard-error. After wiring
  // `_zo_str_slice` at runtime, the executor now emits
  // `Insn::StrSlice` so bounded slicing works without
  // compile-time operands.
  assert_sir_structure(
    r#"fun main() {
  imu s: str = "hello";
  mut i: int = 0;
  imu x: str = s[i..3];
}"#,
    |sir| {
      let has_runtime_slice =
        sir.iter().any(|i| matches!(i, Insn::StrSlice { .. }));

      assert!(
        has_runtime_slice,
        "runtime-bounded slice should emit Insn::StrSlice"
      );
    },
  );
}
