use super::common::{BOOL, F64, Harness, SPAN, U64};

use crate::{FoldResult, Operand};

use zo_sir::BinOp;

// — integer identity: x + 0 → x, 0 + x → x.

#[test]
fn int_add_zero_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, x, zero, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn int_add_zero_lhs() {
  let mut h = Harness::new();
  let zero = h.int(0);
  let x = h.runtime();

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, zero, x, SPAN, U64),
    Some(FoldResult::Forward(Operand::Rhs)),
  );
}

// — integer identity: x - 0 → x.

#[test]
fn int_sub_zero_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Sub, x, zero, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

// — integer identity: x * 1 → x, 1 * x → x.

#[test]
fn int_mul_one_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let one = h.int(1);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, one, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn int_mul_one_lhs() {
  let mut h = Harness::new();
  let one = h.int(1);
  let x = h.runtime();

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, one, x, SPAN, U64),
    Some(FoldResult::Forward(Operand::Rhs)),
  );
}

// — integer absorbing: x * 0 → 0, 0 * x → 0.

#[test]
fn int_mul_zero_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, zero, SPAN, U64),
    Some(FoldResult::Int(0)),
  );
}

#[test]
fn int_mul_zero_lhs() {
  let mut h = Harness::new();
  let zero = h.int(0);
  let x = h.runtime();

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, zero, x, SPAN, U64),
    Some(FoldResult::Int(0)),
  );
}

// — integer identity: x / 1 → x.

#[test]
fn int_div_one_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let one = h.int(1);

  assert_eq!(
    h.fold().fold_binop(BinOp::Div, x, one, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

// — bitwise identity: x | 0 → x, x ^ 0 → x.

#[test]
fn int_bitor_zero_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::BitOr, x, zero, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn int_bitxor_zero_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::BitXor, x, zero, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

// — bitwise absorbing: x & 0 → 0.

#[test]
fn int_bitand_zero_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::BitAnd, x, zero, SPAN, U64),
    Some(FoldResult::Int(0)),
  );
}

#[test]
fn int_bitand_zero_lhs() {
  let mut h = Harness::new();
  let zero = h.int(0);
  let x = h.runtime();

  assert_eq!(
    h.fold().fold_binop(BinOp::BitAnd, zero, x, SPAN, U64),
    Some(FoldResult::Int(0)),
  );
}

// — shift identity: x << 0 → x, x >> 0 → x.

#[test]
fn int_shl_zero() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Shl, x, zero, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn int_shr_zero() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Shr, x, zero, SPAN, U64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

// — float identity (safe): x + 0.0 → x, x * 1.0 → x.

#[test]
fn float_add_zero_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.float(0.0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, x, zero, SPAN, F64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn float_mul_one_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let one = h.float(1.0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, one, SPAN, F64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn float_div_one_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let one = h.float(1.0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Div, x, one, SPAN, F64),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

// — float: x * 0.0 must NOT be simplified (NaN/Inf).

#[test]
fn float_mul_zero_not_simplified() {
  let mut h = Harness::new();
  let x = h.runtime();
  let zero = h.float(0.0);

  assert_eq!(h.fold().fold_binop(BinOp::Mul, x, zero, SPAN, F64), None,);
}

// — boolean identity/absorbing.

#[test]
fn bool_and_true_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let t = h.bool(true);

  assert_eq!(
    h.fold().fold_binop(BinOp::And, x, t, SPAN, BOOL),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn bool_and_false_rhs_absorbing() {
  let mut h = Harness::new();
  let x = h.runtime();
  let f = h.bool(false);

  assert_eq!(
    h.fold().fold_binop(BinOp::And, x, f, SPAN, BOOL),
    Some(FoldResult::Bool(false)),
  );
}

#[test]
fn bool_or_false_rhs() {
  let mut h = Harness::new();
  let x = h.runtime();
  let f = h.bool(false);

  assert_eq!(
    h.fold().fold_binop(BinOp::Or, x, f, SPAN, BOOL),
    Some(FoldResult::Forward(Operand::Lhs)),
  );
}

#[test]
fn bool_or_true_rhs_absorbing() {
  let mut h = Harness::new();
  let x = h.runtime();
  let t = h.bool(true);

  assert_eq!(
    h.fold().fold_binop(BinOp::Or, x, t, SPAN, BOOL),
    Some(FoldResult::Bool(true)),
  );
}

#[test]
fn bool_and_false_lhs_absorbing() {
  let mut h = Harness::new();
  let f = h.bool(false);
  let x = h.runtime();

  assert_eq!(
    h.fold().fold_binop(BinOp::And, f, x, SPAN, BOOL),
    Some(FoldResult::Bool(false)),
  );
}

#[test]
fn bool_or_true_lhs_absorbing() {
  let mut h = Harness::new();
  let t = h.bool(true);
  let x = h.runtime();

  assert_eq!(
    h.fold().fold_binop(BinOp::Or, t, x, SPAN, BOOL),
    Some(FoldResult::Bool(true)),
  );
}

// — two runtime values: no simplification.

#[test]
fn two_runtimes_returns_none() {
  let mut h = Harness::new();
  let x = h.runtime();
  let y = h.runtime();

  assert_eq!(h.fold().fold_binop(BinOp::Add, x, y, SPAN, U64), None,);
}
