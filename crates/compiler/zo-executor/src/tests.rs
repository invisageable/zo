pub(crate) mod common;
pub(crate) mod errors;

use crate::tests::common::{assert_annotations_stream, assert_sir_stream};

use zo_interner::Symbol;
use zo_sir::{BinOp, Insn};
use zo_ty::{IntWidth, Ty, TyId};
use zo_value::ValueId;

#[test]
fn test_integer_literal() {
  assert_annotations_stream(
    "42",
    &[(
      0,
      Ty::Int {
        signed: true,
        width: IntWidth::S32,
      },
      Insn::ConstInt {
        value: 42,
        ty_id: TyId(0),
      },
    )],
  );
}

#[test]
fn test_boolean_literals() {
  assert_annotations_stream(
    "true false",
    &[
      (
        0,
        Ty::Bool,
        Insn::ConstBool {
          value: true,
          ty_id: TyId(0),
        },
      ),
      (
        1,
        Ty::Bool,
        Insn::ConstBool {
          value: false,
          ty_id: TyId(0),
        },
      ),
    ],
  );
}

#[test]
fn test_simple_addition() {
  let s32_ty = Ty::Int {
    signed: true,
    width: IntWidth::S32,
  };

  assert_annotations_stream(
    "1 + 2",
    &[
      (
        0,
        s32_ty,
        Insn::ConstInt {
          value: 1,
          ty_id: TyId(0),
        },
      ),
      (
        1,
        s32_ty,
        Insn::ConstInt {
          value: 2,
          ty_id: TyId(0),
        },
      ),
      (
        2,
        s32_ty,
        Insn::ConstInt {
          value: 3,
          ty_id: TyId(0),
        },
      ),
    ],
  );
}

#[test]
fn test_division_by_zero() {
  let s32_ty = Ty::Int {
    signed: true,
    width: IntWidth::S32,
  };

  // Division by zero should only emit the two constants, no result
  assert_annotations_stream(
    "5 / 0",
    &[
      (
        0,
        s32_ty,
        Insn::ConstInt {
          value: 5,
          ty_id: TyId(0),
        },
      ),
      (
        1, // The second operand is at index 1 in postfix order
        s32_ty,
        Insn::ConstInt {
          value: 0,
          ty_id: TyId(0),
        },
      ),
      // No third instruction - division by zero produces error
    ],
  );
}

#[test]
fn test_large_integer_literal() {
  let s32_ty = Ty::Int {
    signed: true,
    width: IntWidth::S32,
  };

  // Test that we're reading actual values from literal store, not indices
  assert_annotations_stream(
    "12345",
    &[(
      0,
      s32_ty,
      Insn::ConstInt {
        value: 12345, // Should be 12345, not 0 (the index)
        ty_id: TyId(0),
      },
    )],
  );
}

#[test]
fn test_simple_function() {
  assert_sir_stream(
    "fun add(x: int, y: int) -> int { x + y }",
    &[
      // Function definition
      Insn::FunDef {
        name: Symbol(25), // "add" - Symbol IDs vary based on interner state
        params: vec![
          (Symbol(26), TyId(1)), // x: int
          (Symbol(27), TyId(1)), // y: int
        ],
        return_ty: TyId(1),
        body_start: 1,
      },
      // Load x parameter
      Insn::Load {
        dst: ValueId(0),
        src: 0, // First parameter
        ty_id: TyId(1),
      },
      // Load y parameter
      Insn::Load {
        dst: ValueId(1),
        src: 1, // Second parameter
        ty_id: TyId(1),
      },
      // x + y
      Insn::BinOp {
        dst: ValueId(2),
        op: BinOp::Add,
        lhs: ValueId(0), // x (loaded)
        rhs: ValueId(1), // y (loaded)
        ty_id: TyId(1),
      },
      // implicit return
      Insn::Return {
        value: Some(ValueId(2)), // Result of addition
        ty_id: TyId(1),          // int type (matches function return type)
      },
    ],
  );
}

#[test]
fn test_function_call() {
  assert_sir_stream(
    r#"
      fun add(x: int, y: int) -> int { x + y }
      fun main() -> int { add(10, 20) }
    "#,
    &[
      // add function definition
      Insn::FunDef {
        name: Symbol(25), // "add"
        params: vec![
          (Symbol(26), TyId(1)), // x: int
          (Symbol(27), TyId(1)), // y: int
        ],
        return_ty: TyId(1),
        body_start: 1,
      },
      // add body: load x
      Insn::Load {
        dst: ValueId(0),
        src: 0, // x param
        ty_id: TyId(1),
      },
      // add body: load y
      Insn::Load {
        dst: ValueId(1),
        src: 1, // y param
        ty_id: TyId(1),
      },
      // add body: x + y
      Insn::BinOp {
        dst: ValueId(2),
        op: BinOp::Add,
        lhs: ValueId(0), // x loaded
        rhs: ValueId(1), // y loaded
        ty_id: TyId(1),
      },
      // add body: implicit return
      Insn::Return {
        value: Some(ValueId(2)),
        ty_id: TyId(1), // int type
      },
      // main function definition
      Insn::FunDef {
        name: Symbol(28), // "main"
        params: vec![],
        return_ty: TyId(1),
        body_start: 6,
      },
      // main body: 10
      Insn::ConstInt {
        value: 10,
        ty_id: TyId(1),
      },
      // main body: 20
      Insn::ConstInt {
        value: 20,
        ty_id: TyId(1),
      },
      // main body: call add(10, 20)
      Insn::Call {
        name: Symbol(25), // "add"
        args: vec![ValueId(3), ValueId(4)],
        ty_id: TyId(1),
      },
      // main body: implicit return
      Insn::Return {
        value: Some(ValueId(5)),
        ty_id: TyId(1), // int type
      },
    ],
  );
}

#[test]
fn test_main_with_show() {
  assert_sir_stream(
    r#"fun main() { show("hello world") }"#,
    &[
      // main function definition
      Insn::FunDef {
        name: Symbol(25), // "main"
        params: vec![],
        return_ty: TyId(0), // unit (void)
        body_start: 1,
      },
      // main body: "hello world" string literal
      Insn::ConstString {
        symbol: Symbol(27), // "hello world" interned
        ty_id: TyId(1),     // str type
      },
      // main body: call show("hello world")
      // show is an external/builtin function, not defined here
      Insn::Call {
        name: Symbol(26),       // "show"
        args: vec![ValueId(0)], // the string constant
        ty_id: TyId(0),         // unit return type
      },
      // main body: implicit return
      Insn::Return {
        value: None,    // void return
        ty_id: TyId(0), // unit type
      },
    ],
  );
}

#[test]
fn test_function_with_return() {
  assert_sir_stream(
    "fun square(n: int) -> int { return n * n }",
    &[
      // Function definition
      Insn::FunDef {
        name: Symbol(25), // "square"
        params: vec![
          (Symbol(26), TyId(1)), // n: int
        ],
        return_ty: TyId(1),
        body_start: 1,
      },
      // Load n parameter (first use)
      Insn::Load {
        dst: ValueId(0),
        src: 0, // n param
        ty_id: TyId(1),
      },
      // Load n parameter (second use - SSA requires separate loads)
      Insn::Load {
        dst: ValueId(1),
        src: 0, // n param
        ty_id: TyId(1),
      },
      // n * n
      Insn::BinOp {
        dst: ValueId(2),
        op: BinOp::Mul,
        lhs: ValueId(0), // first n load
        rhs: ValueId(1), // second n load
        ty_id: TyId(1),
      },
      // explicit return
      Insn::Return {
        value: Some(ValueId(2)), // Result of multiplication
        ty_id: TyId(1),          // int type
      },
    ],
  );
}

#[test]
#[ignore = "SIR expectations need update"]
fn test_directives() {
  assert_sir_stream(
    r#"
      fun main() {
        imu view: </> ::= <>hello</>;
        #dom view;
      }
    "#,
    &[
      Insn::FunDef {
        name: Symbol(25),
        params: Vec::new(),
        return_ty: TyId(0),
        body_start: 1,
      },
      Insn::Template {
        id: ValueId(3),
        name: None,
        ty_id: TyId(1),
        commands: vec![], // Empty for test
      },
      Insn::Directive {
        name: Symbol(28),
        value: ValueId(4),
        ty_id: TyId(3),
      },
      Insn::Return {
        value: None,
        ty_id: TyId(0),
      },
    ],
  );
}

// #[test]
// fn test_element_fragment() {
//   assert_sir_stream(
//     r#"fun main() {
//       imu view: </> ::= <>hello</>;
//     }"#,
//     &[],
//   );
// }

// #[test]
// fn test_fragment_directive_combo() {
//   assert_sir_stream(
//     r#"fun main() {
//       imu view: </> ::= <>hello</>;

//       #dom view;
//     }"#,
//     &[],
//   );
// }
