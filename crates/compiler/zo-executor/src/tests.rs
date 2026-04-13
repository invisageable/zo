pub(crate) mod arrays;
pub(crate) mod arrays_generic;
pub(crate) mod bitwise;
pub(crate) mod closures;
pub(crate) mod common;
pub(crate) mod concat;
pub(crate) mod constants;
pub(crate) mod control_flow;
pub(crate) mod enums;
pub(crate) mod errors;
pub(crate) mod folding;
pub(crate) mod generics;
pub(crate) mod interpolation;
pub(crate) mod matching;
pub(crate) mod modules;
pub(crate) mod structs;
pub(crate) mod styles;
pub(crate) mod templates;
pub(crate) mod tuples;
pub(crate) mod type_aliases;
pub(crate) mod unary;

use crate::tests::common::{
  assert_annotations_stream, assert_sir_stream, execute_raw,
};

use zo_interner::Symbol;
use zo_sir::{BinOp, Insn, LoadSource};
use zo_ty::{IntWidth, Ty, TyId};
use zo_value::{FunctionKind, Pubness, ValueId};

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
        dst: ValueId(0),
        value: 42,
        ty_id: TyId(8),
      },
    )],
  );
}

#[test]
fn test_bytes_literal() {
  let (insns, _) = execute_raw("fun main() { showln(`z`); }");

  // ConstInt with bytes_type (TyId 5) and value 122 ('z').
  let bytes_const = insns.iter().find(|i| {
    matches!(i, Insn::ConstInt { ty_id, value, .. }
      if ty_id.0 == 5 && *value == 122)
  });

  assert!(
    bytes_const.is_some(),
    "expected ConstInt with ty_id=5 (bytes) and value=122"
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
          dst: ValueId(0),
          value: true,
          ty_id: TyId(2),
        },
      ),
      (
        1,
        Ty::Bool,
        Insn::ConstBool {
          dst: ValueId(1),
          value: false,
          ty_id: TyId(2),
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

  // Operands are folded away (Nop'd). Only the result remains.
  assert_annotations_stream(
    "1 + 2",
    &[(
      2,
      s32_ty,
      Insn::ConstInt {
        dst: ValueId(2),
        value: 3,
        ty_id: TyId(8),
      },
    )],
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
          dst: ValueId(0),
          value: 5,
          ty_id: TyId(8),
        },
      ),
      (
        1, // The second operand is at index 1 in postfix order
        s32_ty,
        Insn::ConstInt {
          dst: ValueId(1),
          value: 0,
          ty_id: TyId(8),
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
        dst: ValueId(0),
        value: 12345, // Should be 12345, not 0 (the index)
        ty_id: TyId(8),
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
          (Symbol(26), TyId(8)), // x: int
          (Symbol(27), TyId(8)), // y: int
        ],
        return_ty: TyId(8),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      // Load x parameter
      Insn::Load {
        dst: ValueId(0),
        src: LoadSource::Param(0),
        ty_id: TyId(8),
      },
      // Load y parameter
      Insn::Load {
        dst: ValueId(1),
        src: LoadSource::Param(1),
        ty_id: TyId(8),
      },
      // x + y
      Insn::BinOp {
        dst: ValueId(2),
        op: BinOp::Add,
        lhs: ValueId(0), // x (loaded)
        rhs: ValueId(1), // y (loaded)
        ty_id: TyId(8),
      },
      // implicit return
      Insn::Return {
        value: Some(ValueId(2)), // Result of addition
        ty_id: TyId(8),          // int type (matches function return type)
      },
    ],
  );
}

#[test]
fn test_function_call() {
  assert_sir_stream(
    r#"
      fun add(x: int, y: int) -> int { x + y }
      fun main() { add(10, 20); }
    "#,
    &[
      // add function definition
      Insn::FunDef {
        name: Symbol(25), // "add"
        params: vec![
          (Symbol(26), TyId(8)), // x: int
          (Symbol(27), TyId(8)), // y: int
        ],
        return_ty: TyId(8),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      // add body: load x
      Insn::Load {
        dst: ValueId(0),
        src: LoadSource::Param(0),
        ty_id: TyId(8),
      },
      // add body: load y
      Insn::Load {
        dst: ValueId(1),
        src: LoadSource::Param(1),
        ty_id: TyId(8),
      },
      // add body: x + y
      Insn::BinOp {
        dst: ValueId(2),
        op: BinOp::Add,
        lhs: ValueId(0), // x loaded
        rhs: ValueId(1), // y loaded
        ty_id: TyId(8),
      },
      // add body: implicit return
      Insn::Return {
        value: Some(ValueId(2)),
        ty_id: TyId(8), // int type
      },
      // main function definition
      Insn::FunDef {
        name: Symbol(28), // "main"
        params: vec![],
        return_ty: TyId(1),
        body_start: 6,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      // main body: 10
      Insn::ConstInt {
        dst: ValueId(3),
        value: 10,
        ty_id: TyId(8),
      },
      // main body: 20
      Insn::ConstInt {
        dst: ValueId(4),
        value: 20,
        ty_id: TyId(8),
      },
      // main body: call add(10, 20)
      Insn::Call {
        dst: ValueId(5),
        name: Symbol(25), // "add"
        args: vec![ValueId(3), ValueId(4)],
        ty_id: TyId(8),
      },
      // main body: return void
      Insn::Return {
        value: None,
        ty_id: TyId(1),
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
        return_ty: TyId(1), // unit (void)
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      // main body: "hello world" string literal
      Insn::ConstString {
        dst: ValueId(0),
        symbol: Symbol(27), // "hello world" interned
        ty_id: TyId(4),     // str type
      },
      // main body: call show("hello world")
      // show is an external/builtin function, not defined here
      Insn::Call {
        dst: ValueId(1),
        name: Symbol(26),       // "show"
        args: vec![ValueId(0)], // the string constant
        ty_id: TyId(1),         // unit return type
      },
      // main body: implicit return
      Insn::Return {
        value: None,    // void return
        ty_id: TyId(1), // unit type
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
          (Symbol(26), TyId(8)), // n: int
        ],
        return_ty: TyId(8),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      // Load n parameter (first use)
      Insn::Load {
        dst: ValueId(0),
        src: LoadSource::Param(0),
        ty_id: TyId(8),
      },
      // Load n parameter (second use - SSA requires separate loads)
      Insn::Load {
        dst: ValueId(1),
        src: LoadSource::Param(0),
        ty_id: TyId(8),
      },
      // n * n
      Insn::BinOp {
        dst: ValueId(2),
        op: BinOp::Mul,
        lhs: ValueId(0), // first n load
        rhs: ValueId(1), // second n load
        ty_id: TyId(8),
      },
      // explicit return
      Insn::Return {
        value: Some(ValueId(2)), // Result of multiplication
        ty_id: TyId(8),          // int type
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
        return_ty: TyId(1),
        body_start: 1,
        kind: FunctionKind::UserDefined,
        pubness: Pubness::No,
      },
      Insn::Template {
        id: ValueId(3),
        name: None,
        ty_id: TyId(18),
        commands: vec![], // Empty for test
        bindings: zo_sir::TemplateBindings::default(),
      },
      Insn::Directive {
        name: Symbol(28),
        value: ValueId(4),
        ty_id: TyId(3),
      },
      Insn::Return {
        value: None,
        ty_id: TyId(1),
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
