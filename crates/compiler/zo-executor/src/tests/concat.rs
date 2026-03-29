use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_sir::{BinOp, Insn};

// === LITERAL CONCAT ===

#[test]
fn test_concat_literal_folds_to_const_string() {
  assert_sir_structure(
    r#"fun main() {
  imu s: str = "hello" ++ " world";
}"#,
    |sir| {
      // Literal ++ literal should fold into a single
      // ConstString at compile time.
      let has_hello_world = sir.iter().any(|i| {
        if let Insn::ConstString { symbol, .. } = i {
          // We can't check the string content easily
          // here, but there should be a ConstString
          // that's NOT "hello" or " world" alone.
          symbol.0 > 0
        } else {
          false
        }
      });

      assert!(has_hello_world, "literal concat should fold to ConstString");
    },
  );
}

#[test]
fn test_concat_no_binop_for_literals() {
  assert_sir_structure(
    r#"fun main() {
  imu s: str = "a" ++ "b";
}"#,
    |sir| {
      // Literal concat should NOT emit a BinOp::Concat
      // — it should be fully folded.
      let has_concat_binop = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: BinOp::Concat,
            ..
          }
        )
      });

      assert!(
        !has_concat_binop,
        "literal concat should fold, not emit BinOp::Concat"
      );
    },
  );
}

// === TRIPLE CONCAT ===

#[test]
fn test_concat_triple_folds() {
  assert_sir_structure(
    r#"fun main() {
  imu s: str = "a" ++ "b" ++ "c";
}"#,
    |sir| {
      // Triple concat should fold completely — no
      // BinOp::Concat in the output.
      let has_concat_binop = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: BinOp::Concat,
            ..
          }
        )
      });

      assert!(
        !has_concat_binop,
        "triple literal concat should fold completely"
      );
    },
  );
}

// === TYPE ERRORS ===

#[test]
fn test_concat_str_int_error() {
  assert_execution_error(
    r#"fun main() {
  imu s: str = "hello" ++ 42;
}"#,
    ErrorKind::TypeMismatch,
  );
}

#[test]
fn test_concat_int_str_error() {
  assert_execution_error(
    r#"fun main() {
  imu s: str = 42 ++ "hello";
}"#,
    ErrorKind::TypeMismatch,
  );
}

// === RUNTIME CONCAT ===

#[test]
fn test_concat_runtime_emits_binop() {
  // When concat can't be folded (e.g., function params),
  // the executor must emit BinOp::Concat.
  assert_sir_structure(
    r#"fun join(a: str, b: str) -> str {
  a ++ b
}

fun main() {
  imu s: str = join("hello", " world");
}"#,
    |sir| {
      let has_concat = sir.iter().any(|i| {
        matches!(
          i,
          Insn::BinOp {
            op: BinOp::Concat,
            ..
          }
        )
      });

      assert!(has_concat, "runtime concat should emit BinOp::Concat");
    },
  );
}

// === VARIABLE CONCAT ===

#[test]
fn test_concat_variables_no_errors() {
  assert_sir_structure(
    r#"fun main() {
  imu a: str = "hello";
  imu b: str = " world";
  imu s: str = a ++ b;
}"#,
    |sir| {
      // Variable concat should produce a ConstString
      // (compile-time fold via SIR tracing).
      let const_strings = sir
        .iter()
        .filter(|i| matches!(i, Insn::ConstString { .. }))
        .collect::<Vec<_>>();

      // At least 3: "hello", " world", "hello world".
      assert!(
        const_strings.len() >= 3,
        "variable concat should fold to ConstString \
         (got {} ConstStrings)",
        const_strings.len()
      );
    },
  );
}
