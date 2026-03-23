use crate::tests::common::assert_sir_structure;

use zo_sir::Insn;

#[test]
fn test_interp_desugars_to_show_calls() {
  // showln("{x}") with an int variable should desugar
  // into show(x) + showln("") or showln(x).
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 42;
  showln("{x}");
}"#,
    |sir| {
      // The desugared output should contain Call
      // instructions for show/showln.
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .collect::<Vec<_>>();

      assert!(
        !calls.is_empty(),
        "expected show/showln Call instructions \
         from interpolation desugaring, got none"
      );
    },
  );
}

#[test]
fn test_interp_multi_var() {
  // showln("{x}, {y}") should desugar into:
  // show(x) + show(", ") + showln(y)
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 1;
  imu y: int = 2;
  showln("{x}, {y}");
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .collect::<Vec<_>>();

      // At least 3 calls: show(x), show(", "), showln(y)
      assert!(
        calls.len() >= 3,
        "expected >= 3 Call instructions for \
         multi-var interpolation, got {}",
        calls.len()
      );
    },
  );
}

#[test]
fn test_interp_literal_segments() {
  // showln("a{x}b") should produce:
  // show("a") + show(x) + showln("b")
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 42;
  showln("a{x}b");
}"#,
    |sir| {
      // Should have ConstString instructions for "a"
      // and "b" segments.
      let const_strings = sir
        .iter()
        .filter(|i| matches!(i, Insn::ConstString { .. }))
        .collect::<Vec<_>>();

      // At least 2 literal segments ("a" and "b") +
      // the dead full format string.
      assert!(
        const_strings.len() >= 2,
        "expected >= 2 ConstString for literal \
         segments, got {}",
        const_strings.len()
      );
    },
  );
}

#[test]
fn test_plain_string_no_desugaring() {
  // showln("hello") with no {} should NOT desugar —
  // should produce exactly 1 Call.
  assert_sir_structure(
    r#"fun main() {
  showln("hello");
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .collect::<Vec<_>>();

      assert_eq!(
        calls.len(),
        1,
        "plain string should produce exactly 1 \
         Call, got {}",
        calls.len()
      );
    },
  );
}

#[test]
fn test_interp_single_var_only() {
  // showln("{x}") — single variable, no surrounding text.
  // Should desugar to just showln(x).
  assert_sir_structure(
    r#"fun main() {
  imu x: int = 7;
  showln("{x}");
}"#,
    |sir| {
      let calls = sir
        .iter()
        .filter(|i| matches!(i, Insn::Call { .. }))
        .collect::<Vec<_>>();

      // Just 1 call: showln(x).
      assert_eq!(
        calls.len(),
        1,
        "single-var interpolation should produce \
         1 Call, got {}",
        calls.len()
      );
    },
  );
}
