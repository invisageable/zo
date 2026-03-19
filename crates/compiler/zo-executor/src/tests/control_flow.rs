use crate::tests::common::{assert_execution_error, assert_sir_stream};

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
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
      Insn::Label { id: 0 },
    ],
  );
}

#[test]
fn test_if_else() {
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
      Insn::Return {
        value: Some(ValueId(1)),
        ty_id: TyId(8),
      },
      Insn::Label { id: 0 },
      Insn::ConstInt {
        value: 2,
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
      Insn::Return {
        value: None,
        ty_id: TyId(1),
      },
      Insn::Jump { target: 0 },
      Insn::Label { id: 1 },
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
