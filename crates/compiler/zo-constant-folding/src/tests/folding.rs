use super::common::{BOOL, F64, Harness, S64, SPAN, U64};

use crate::FoldResult;

use zo_sir::{BinOp, UnOp};

// — integer arithmetic.

#[test]
fn int_add() {
  let mut h = Harness::new();
  let a = h.int(2);
  let b = h.int(3);

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, a, b, SPAN, U64),
    Some(FoldResult::Int(5)),
  );
}

#[test]
fn int_sub() {
  let mut h = Harness::new();
  let a = h.int(10);
  let b = h.int(4);

  assert_eq!(
    h.fold().fold_binop(BinOp::Sub, a, b, SPAN, U64),
    Some(FoldResult::Int(6)),
  );
}

#[test]
fn int_mul() {
  let mut h = Harness::new();
  let a = h.int(7);
  let b = h.int(6);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, a, b, SPAN, U64),
    Some(FoldResult::Int(42)),
  );
}

#[test]
fn int_div() {
  let mut h = Harness::new();
  let a = h.int(42);
  let b = h.int(6);

  assert_eq!(
    h.fold().fold_binop(BinOp::Div, a, b, SPAN, U64),
    Some(FoldResult::Int(7)),
  );
}

#[test]
fn int_rem() {
  let mut h = Harness::new();
  let a = h.int(10);
  let b = h.int(3);

  assert_eq!(
    h.fold().fold_binop(BinOp::Rem, a, b, SPAN, U64),
    Some(FoldResult::Int(1)),
  );
}

// — integer comparisons.

#[test]
fn int_eq_true() {
  let mut h = Harness::new();
  let a = h.int(5);
  let b = h.int(5);

  assert_eq!(
    h.fold().fold_binop(BinOp::Eq, a, b, SPAN, U64),
    Some(FoldResult::Bool(true)),
  );
}

#[test]
fn int_eq_false() {
  let mut h = Harness::new();
  let a = h.int(5);
  let b = h.int(6);

  assert_eq!(
    h.fold().fold_binop(BinOp::Eq, a, b, SPAN, U64),
    Some(FoldResult::Bool(false)),
  );
}

#[test]
fn int_lt() {
  let mut h = Harness::new();
  let a = h.int(3);
  let b = h.int(5);

  assert_eq!(
    h.fold().fold_binop(BinOp::Lt, a, b, SPAN, U64),
    Some(FoldResult::Bool(true)),
  );
}

#[test]
fn int_gte() {
  let mut h = Harness::new();
  let a = h.int(5);
  let b = h.int(5);

  assert_eq!(
    h.fold().fold_binop(BinOp::Gte, a, b, SPAN, U64),
    Some(FoldResult::Bool(true)),
  );
}

// — integer bitwise.

#[test]
fn int_bit_and() {
  let mut h = Harness::new();
  let a = h.int(0b1100);
  let b = h.int(0b1010);

  assert_eq!(
    h.fold().fold_binop(BinOp::BitAnd, a, b, SPAN, U64),
    Some(FoldResult::Int(0b1000)),
  );
}

#[test]
fn int_bit_or() {
  let mut h = Harness::new();
  let a = h.int(0b1100);
  let b = h.int(0b1010);

  assert_eq!(
    h.fold().fold_binop(BinOp::BitOr, a, b, SPAN, U64),
    Some(FoldResult::Int(0b1110)),
  );
}

#[test]
fn int_bit_xor() {
  let mut h = Harness::new();
  let a = h.int(0b1100);
  let b = h.int(0b1010);

  assert_eq!(
    h.fold().fold_binop(BinOp::BitXor, a, b, SPAN, U64),
    Some(FoldResult::Int(0b0110)),
  );
}

#[test]
fn int_shl() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.int(4);

  assert_eq!(
    h.fold().fold_binop(BinOp::Shl, a, b, SPAN, U64),
    Some(FoldResult::Int(16)),
  );
}

#[test]
fn int_shr() {
  let mut h = Harness::new();
  let a = h.int(16);
  let b = h.int(4);

  assert_eq!(
    h.fold().fold_binop(BinOp::Shr, a, b, SPAN, U64),
    Some(FoldResult::Int(1)),
  );
}

#[test]
fn int_shl_boundary_63() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.int(63);

  assert_eq!(
    h.fold().fold_binop(BinOp::Shl, a, b, SPAN, U64),
    Some(FoldResult::Int(1 << 63)),
  );
}

// — float arithmetic.

#[test]
fn float_add() {
  let mut h = Harness::new();
  let a = h.float(1.5);
  let b = h.float(2.5);

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, a, b, SPAN, F64),
    Some(FoldResult::Float(4.0)),
  );
}

#[test]
fn float_mul() {
  let mut h = Harness::new();
  let a = h.float(3.0);
  let b = h.float(4.0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, a, b, SPAN, F64),
    Some(FoldResult::Float(12.0)),
  );
}

#[test]
fn float_div() {
  let mut h = Harness::new();
  let a = h.float(10.0);
  let b = h.float(4.0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Div, a, b, SPAN, F64),
    Some(FoldResult::Float(2.5)),
  );
}

#[test]
fn float_lt() {
  let mut h = Harness::new();
  let a = h.float(1.0);
  let b = h.float(2.0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Lt, a, b, SPAN, F64),
    Some(FoldResult::Bool(true)),
  );
}

// — boolean logic.

#[test]
fn bool_and_true() {
  let mut h = Harness::new();
  let a = h.bool(true);
  let b = h.bool(true);

  assert_eq!(
    h.fold().fold_binop(BinOp::And, a, b, SPAN, BOOL),
    Some(FoldResult::Bool(true)),
  );
}

#[test]
fn bool_and_false() {
  let mut h = Harness::new();
  let a = h.bool(true);
  let b = h.bool(false);

  assert_eq!(
    h.fold().fold_binop(BinOp::And, a, b, SPAN, BOOL),
    Some(FoldResult::Bool(false)),
  );
}

#[test]
fn bool_or() {
  let mut h = Harness::new();
  let a = h.bool(false);
  let b = h.bool(true);

  assert_eq!(
    h.fold().fold_binop(BinOp::Or, a, b, SPAN, BOOL),
    Some(FoldResult::Bool(true)),
  );
}

#[test]
fn bool_eq() {
  let mut h = Harness::new();
  let a = h.bool(true);
  let b = h.bool(true);

  assert_eq!(
    h.fold().fold_binop(BinOp::Eq, a, b, SPAN, BOOL),
    Some(FoldResult::Bool(true)),
  );
}

#[test]
fn bool_neq() {
  let mut h = Harness::new();
  let a = h.bool(true);
  let b = h.bool(false);

  assert_eq!(
    h.fold().fold_binop(BinOp::Neq, a, b, SPAN, BOOL),
    Some(FoldResult::Bool(true)),
  );
}

// — unary operations.

#[test]
fn neg_int_positive() {
  let mut h = Harness::new();
  let a = h.int(5);

  assert_eq!(
    h.fold().fold_unop(UnOp::Neg, a, SPAN, S64),
    // -5 in two's complement u64.
    Some(FoldResult::Int((-5i64) as u64)),
  );
}

#[test]
fn neg_int_zero() {
  let mut h = Harness::new();
  let a = h.int(0);

  assert_eq!(
    h.fold().fold_unop(UnOp::Neg, a, SPAN, S64),
    Some(FoldResult::Int(0)),
  );
}

#[test]
fn neg_float() {
  let mut h = Harness::new();
  let a = h.float(2.78);

  assert_eq!(
    h.fold().fold_unop(UnOp::Neg, a, SPAN, F64),
    Some(FoldResult::Float(-2.78)),
  );
}

#[test]
fn not_bool() {
  let mut h = Harness::new();
  let a = h.bool(true);

  assert_eq!(
    h.fold().fold_unop(UnOp::Not, a, SPAN, BOOL),
    Some(FoldResult::Bool(false)),
  );
}

#[test]
fn bit_not_int() {
  let mut h = Harness::new();
  let a = h.int(0);

  assert_eq!(
    h.fold().fold_unop(UnOp::BitNot, a, SPAN, U64),
    Some(FoldResult::Int(u64::MAX)),
  );
}

// — identity operations (edge cases).

#[test]
fn int_add_zero() {
  let mut h = Harness::new();
  let a = h.int(42);
  let b = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, a, b, SPAN, U64),
    Some(FoldResult::Int(42)),
  );
}

#[test]
fn int_mul_one() {
  let mut h = Harness::new();
  let a = h.int(42);
  let b = h.int(1);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, a, b, SPAN, U64),
    Some(FoldResult::Int(42)),
  );
}

#[test]
fn int_mul_zero() {
  let mut h = Harness::new();
  let a = h.int(42);
  let b = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, a, b, SPAN, U64),
    Some(FoldResult::Int(0)),
  );
}

// — type mismatch (int vs float) returns None.

#[test]
fn mixed_int_float_returns_none() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.float(2.0);

  assert_eq!(h.fold().fold_binop(BinOp::Add, a, b, SPAN, U64), None,);
}

// — unsupported ops return None.

#[test]
fn float_bitwise_returns_none() {
  let mut h = Harness::new();
  let a = h.float(1.0);
  let b = h.float(2.0);

  assert_eq!(h.fold().fold_binop(BinOp::BitAnd, a, b, SPAN, F64), None,);
}

#[test]
fn bool_add_returns_none() {
  let mut h = Harness::new();
  let a = h.bool(true);
  let b = h.bool(false);

  assert_eq!(h.fold().fold_binop(BinOp::Add, a, b, SPAN, BOOL), None,);
}
