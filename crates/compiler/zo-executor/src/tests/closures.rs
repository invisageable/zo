use crate::tests::common::assert_sir_structure;

use zo_sir::Insn;
use zo_value::FunctionKind;

#[test]
fn test_closure_block_emits_fundef() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int { x + x };
}"#,
    |sir| {
      let closure_count = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::FunDef {
              kind: FunctionKind::Closure { .. },
              ..
            }
          )
        })
        .count();

      assert_eq!(
        closure_count, 1,
        "expected 1 closure FunDef, got {closure_count}"
      );
    },
  );
}

#[test]
fn test_closure_inline_emits_fundef() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x + x;
}"#,
    |sir| {
      let closure_count = sir
        .iter()
        .filter(|i| {
          matches!(
            i,
            Insn::FunDef {
              kind: FunctionKind::Closure { .. },
              ..
            }
          )
        })
        .count();

      assert_eq!(
        closure_count, 1,
        "expected 1 closure FunDef, got {closure_count}"
      );
    },
  );
}

#[test]
fn test_closure_has_return() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x;
}"#,
    |sir| {
      // Closure FunDef should be followed by body + Return.
      let has_closure = sir.iter().any(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      assert!(has_closure, "expected closure FunDef");

      // At least one Return inside the closure body.
      let return_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Return { .. }))
        .count();

      // main's implicit return + closure's return.
      assert!(
        return_count >= 2,
        "expected at least 2 Returns (main + closure), got {return_count}"
      );
    },
  );
}

#[test]
fn test_closure_capture_adds_params() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu y: int = 10;
  imu f := fn(x: int) -> int => x + y;
  f(5)
}"#,
    |sir| {
      // The closure should have capture_count > 0.
      let closure = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      assert!(closure.is_some(), "expected closure FunDef");

      if let Some(Insn::FunDef {
        kind: FunctionKind::Closure { capture_count },
        params,
        ..
      }) = closure
      {
        assert_eq!(
          *capture_count, 1,
          "expected 1 capture, got {capture_count}"
        );

        // Total params = 1 capture (y) + 1 user param (x) = 2.
        assert_eq!(
          params.len(),
          2,
          "expected 2 total params (capture + user), got {}",
          params.len()
        );
      }
    },
  );
}

#[test]
fn test_closure_no_capture_zero_count() {
  assert_sir_structure(
    r#"fun main() {
  imu f := fn(x: int) -> int => x + x;
}"#,
    |sir| {
      let closure = sir.iter().find(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      if let Some(Insn::FunDef {
        kind: FunctionKind::Closure { capture_count },
        params,
        ..
      }) = closure
      {
        assert_eq!(
          *capture_count, 0,
          "expected 0 captures, got {capture_count}"
        );
        assert_eq!(params.len(), 1, "expected 1 param, got {}", params.len());
      }
    },
  );
}

#[test]
fn test_closure_call_emits_call_insn() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu f := fn(x: int) -> int => x + x;
  f(5)
}"#,
    |sir| {
      let has_call = sir.iter().any(|i| matches!(i, Insn::Call { .. }));

      assert!(has_call, "expected Call instruction for closure invocation");
    },
  );
}
