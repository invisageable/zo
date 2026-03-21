use crate::tests::common::{
  assert_execution_error, assert_sir_stream, assert_sir_structure,
};

use zo_error::ErrorKind;
use zo_interner::Symbol;
use zo_sir::Insn;
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
        is_intrinsic: false,
        is_pub: false,
      },
      Insn::ConstBool {
        value: true,
        ty_id: TyId(2),
      },
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 1,
      },
      Insn::ConstInt {
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
  // Get actual output first to update expectations.
  assert_sir_stream(
    r#"fun main() -> int {
  if true {
    1
  } else {
    2
  }
}"#,
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: vec![],
        return_ty: TyId(8),
        body_start: 1,
        is_intrinsic: false,
        is_pub: false,
      },
      Insn::ConstBool {
        value: true,
        ty_id: TyId(2),
      },
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 1,
      },
      Insn::ConstInt {
        value: 1,
        ty_id: TyId(8),
      },
      Insn::Jump { target: 0 },
      Insn::Label { id: 1 },
      Insn::ConstInt {
        value: 2,
        ty_id: TyId(8),
      },
      Insn::Label { id: 0 },
      Insn::Return {
        value: Some(ValueId(2)),
        ty_id: TyId(8),
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
        is_intrinsic: false,
        is_pub: false,
      },
      Insn::Label { id: 0 },
      Insn::ConstBool {
        value: true,
        ty_id: TyId(2),
      },
      Insn::BranchIfNot {
        cond: ValueId(0),
        target: 1,
      },
      Insn::ConstInt {
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
        is_intrinsic: false,
        is_pub: false,
      },
      Insn::ConstInt {
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
fn test_void_function_with_value_is_type_error() {
  assert_execution_error("fun foo() { 42 }", ErrorKind::TypeMismatch);
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
        sir
          .iter()
          .any(|i| matches!(i, Insn::Load { src, .. } if *src >= 100)),
        "expected Load from mutable slot (src >= 100)"
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
