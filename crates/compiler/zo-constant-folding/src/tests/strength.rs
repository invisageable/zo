use super::common::{Harness, SPAN, U64};

use crate::FoldResult;

use zo_sir::BinOp;
use zo_ty::{IntWidth, Ty};

const U32: Ty = Ty::Int {
  signed: false,
  width: IntWidth::U32,
};
const S64: Ty = Ty::Int {
  signed: true,
  width: IntWidth::S64,
};

// — x * 2^n → Shl(x, n).

#[test]
fn mul_by_2() {
  let mut h = Harness::new();
  let x = h.runtime();
  let two = h.int(2);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, two, SPAN, U64),
    Some(FoldResult::Strength(BinOp::Shl, 1)),
  );
}

#[test]
fn mul_by_8() {
  let mut h = Harness::new();
  let x = h.runtime();
  let eight = h.int(8);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, eight, SPAN, U64),
    Some(FoldResult::Strength(BinOp::Shl, 3)),
  );
}

#[test]
fn mul_by_1024() {
  let mut h = Harness::new();
  let x = h.runtime();
  let k = h.int(1024);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, k, SPAN, U64),
    Some(FoldResult::Strength(BinOp::Shl, 10)),
  );
}

// — mul by 1 is identity (handled by algebraic simplification,
//   not strength reduction).

#[test]
fn mul_by_1_is_identity_not_shift() {
  let mut h = Harness::new();
  let x = h.runtime();
  let one = h.int(1);

  // should be Forward(Lhs), not Strength(Shl, 0).
  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, one, SPAN, U64),
    Some(FoldResult::Forward(crate::Operand::Lhs)),
  );
}

// — mul by non-power-of-2 is not reduced.

#[test]
fn mul_by_3_not_reduced() {
  let mut h = Harness::new();
  let x = h.runtime();
  let three = h.int(3);

  assert_eq!(h.fold().fold_binop(BinOp::Mul, x, three, SPAN, U64), None,);
}

// — signed mul by power of 2 is still reduced (shift works).

#[test]
fn signed_mul_by_4() {
  let mut h = Harness::new();
  let x = h.runtime();
  let four = h.int(4);

  assert_eq!(
    h.fold().fold_binop(BinOp::Mul, x, four, SPAN, S64),
    Some(FoldResult::Strength(BinOp::Shl, 2)),
  );
}

// — x / 2^n → Shr(x, n) (unsigned only).

#[test]
fn unsigned_div_by_4() {
  let mut h = Harness::new();
  let x = h.runtime();
  let four = h.int(4);

  assert_eq!(
    h.fold().fold_binop(BinOp::Div, x, four, SPAN, U64),
    Some(FoldResult::Strength(BinOp::Shr, 2)),
  );
}

#[test]
fn signed_div_by_4_not_reduced() {
  let mut h = Harness::new();
  let x = h.runtime();
  let four = h.int(4);

  // signed division by power of 2 is NOT equivalent to
  // arithmetic shift right (rounding differs for negatives).
  assert_eq!(h.fold().fold_binop(BinOp::Div, x, four, SPAN, S64), None,);
}

// — x % 2^n → BitAnd(x, 2^n - 1) (unsigned only).

#[test]
fn unsigned_rem_by_8() {
  let mut h = Harness::new();
  let x = h.runtime();
  let eight = h.int(8);

  assert_eq!(
    h.fold().fold_binop(BinOp::Rem, x, eight, SPAN, U64),
    Some(FoldResult::Strength(BinOp::BitAnd, 7)),
  );
}

#[test]
fn unsigned_rem_by_256() {
  let mut h = Harness::new();
  let x = h.runtime();
  let n = h.int(256);

  assert_eq!(
    h.fold().fold_binop(BinOp::Rem, x, n, SPAN, U32),
    Some(FoldResult::Strength(BinOp::BitAnd, 255)),
  );
}

#[test]
fn signed_rem_by_8_not_reduced() {
  let mut h = Harness::new();
  let x = h.runtime();
  let eight = h.int(8);

  assert_eq!(h.fold().fold_binop(BinOp::Rem, x, eight, SPAN, S64), None,);
}

// — div/rem by 1 is identity/zero (handled by algebraic
//   simplification, not strength reduction).

#[test]
fn div_by_1_is_identity() {
  let mut h = Harness::new();
  let x = h.runtime();
  let one = h.int(1);

  assert_eq!(
    h.fold().fold_binop(BinOp::Div, x, one, SPAN, U64),
    Some(FoldResult::Forward(crate::Operand::Lhs)),
  );
}
