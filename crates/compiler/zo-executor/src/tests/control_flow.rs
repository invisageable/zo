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

#[test]
fn test_for_loop_sum() {
  // for i := 0..5 desugars to:
  //   mut i = 0; while i < 5 { body; i = i + 1; }
  assert_sir_structure(
    r#"fun main() -> int {
  mut sum: int = 0;
  for i := 0..5 {
    sum = sum + i;
  }
  return sum;
}"#,
    |sir| {
      // Must have loop structure: Label, BranchIfNot, Jump
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

      // Must have Stores for sum and the loop variable i
      let store_count = sir
        .iter()
        .filter(|i| matches!(i, Insn::Store { .. }))
        .count();

      assert!(
        store_count >= 3,
        "expected >= 3 Store instructions, got {store_count}"
      );
      // Must have a Return with a value
      assert!(
        sir
          .iter()
          .any(|i| matches!(i, Insn::Return { value: Some(_), .. })),
        "expected Return with value (for)"
      );
    },
  );
}

#[test]
fn test_float_param_produces_load() {
  assert_sir_structure(
    r#"fun add_f(a: float, b: float) -> float {
  return a + b;
}"#,
    |sir| {
      let loads = sir
        .iter()
        .filter(|i| matches!(i, Insn::Load { .. }))
        .collect::<Vec<_>>();

      assert!(
        loads.len() >= 2,
        "expected >= 2 Load for float params, got {}",
        loads.len()
      );
      assert!(
        sir.iter().any(|i| matches!(
          i,
          Insn::BinOp {
            op: zo_sir::BinOp::Add,
            ..
          }
        )),
        "expected BinOp Add for a + b"
      );
    },
  );
}

#[test]
fn test_break_in_while() {
  assert_sir_structure(
    r#"fun main() -> int {
  mut i: int = 0;
  while i < 100 {
    if i == 5 {
      break;
    }
    i = i + 1;
  }
  return i;
}"#,
    |sir| {
      // break emits a Jump to end_label.
      let jumps = sir
        .iter()
        .filter(|i| matches!(i, Insn::Jump { .. }))
        .count();

      assert!(
        jumps >= 2,
        "expected >= 2 Jumps (break + loop-back), got {jumps}"
      );
    },
  );
}

#[test]
fn test_continue_in_for() {
  assert_sir_structure(
    r#"fun main() -> int {
  mut sum: int = 0;
  for i := 0..10 {
    if i == 3 {
      continue;
    }
    sum = sum + i;
  }
  return sum;
}"#,
    |sir| {
      // continue emits a Jump to loop_label.
      let jumps = sir
        .iter()
        .filter(|i| matches!(i, Insn::Jump { .. }))
        .count();

      assert!(
        jumps >= 2,
        "expected >= 2 Jumps (continue + loop-back), got {jumps}"
      );
    },
  );
}

#[test]
fn test_array_literal_and_index() {
  assert_sir_structure(
    r#"fun main() -> int {
  imu arr: []int = [10, 20, 30];
  return arr[1];
}"#,
    |sir| {
      assert!(
        sir.iter().any(|i| matches!(i, Insn::ArrayLiteral { .. })),
        "expected ArrayLiteral instruction"
      );
      assert!(
        sir.iter().any(|i| matches!(i, Insn::ArrayIndex { .. })),
        "expected ArrayIndex instruction"
      );
    },
  );
}

#[test]
fn test_showln_int_emits_call() {
  // showln(42) should emit a Call instruction with an
  // int argument (not just a string).
  assert_sir_structure(
    r#"fun main() {
  showln(42);
}"#,
    |sir| {
      assert!(
        sir.iter().any(|i| matches!(
          i,
          Insn::Call { args, .. } if !args.is_empty()
        )),
        "expected Call with argument for showln(42)"
      );
    },
  );
}
