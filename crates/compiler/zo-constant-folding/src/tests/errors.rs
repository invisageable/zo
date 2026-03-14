use super::common::{F64, Harness, S64, SPAN, U64};

use crate::FoldResult;

use zo_error::{Error, ErrorKind};
use zo_sir::{BinOp, UnOp};

fn is_error(result: Option<FoldResult>, kind: ErrorKind) -> bool {
  matches!(
    result,
    Some(FoldResult::Error(err)) if err == Error::new(kind, SPAN)
  )
}

// — integer overflow.

#[test]
fn int_add_overflow() {
  let mut h = Harness::new();
  let a = h.int(u64::MAX);
  let b = h.int(1);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Add, a, b, SPAN, U64),
    ErrorKind::IntegerOverflow,
  ));
}

#[test]
fn int_sub_underflow() {
  let mut h = Harness::new();
  let a = h.int(0);
  let b = h.int(1);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Sub, a, b, SPAN, U64),
    ErrorKind::IntegerOverflow,
  ));
}

#[test]
fn int_mul_overflow() {
  let mut h = Harness::new();
  let a = h.int(u64::MAX);
  let b = h.int(2);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Mul, a, b, SPAN, U64),
    ErrorKind::IntegerOverflow,
  ));
}

// — division by zero.

#[test]
fn int_div_by_zero() {
  let mut h = Harness::new();
  let a = h.int(42);
  let b = h.int(0);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Div, a, b, SPAN, U64),
    ErrorKind::DivisionByZero,
  ));
}

#[test]
fn int_rem_by_zero() {
  let mut h = Harness::new();
  let a = h.int(42);
  let b = h.int(0);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Rem, a, b, SPAN, U64),
    ErrorKind::RemainderByZero,
  ));
}

#[test]
fn float_div_by_zero() {
  let mut h = Harness::new();
  let a = h.float(1.0);
  let b = h.float(0.0);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Div, a, b, SPAN, F64),
    ErrorKind::DivisionByZero,
  ));
}

// — shift amount too large.

#[test]
fn shl_amount_too_large() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.int(64);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Shl, a, b, SPAN, U64),
    ErrorKind::ShiftAmountTooLarge,
  ));
}

#[test]
fn shr_amount_too_large() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.int(65);

  assert!(is_error(
    h.fold().fold_binop(BinOp::Shr, a, b, SPAN, U64),
    ErrorKind::ShiftAmountTooLarge,
  ));
}

// — negation overflow (i64::MIN has no positive counterpart).

#[test]
fn neg_overflow_i64_min() {
  let mut h = Harness::new();
  // i64::MIN as u64 = 0x8000_0000_0000_0000.
  let a = h.int(i64::MIN as u64);

  assert!(is_error(
    h.fold().fold_unop(UnOp::Neg, a, SPAN, S64),
    ErrorKind::IntegerOverflow,
  ));
}

// — runtime values produce None (not foldable).

#[test]
fn runtime_binop_returns_none() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.runtime();

  assert_eq!(h.fold().fold_binop(BinOp::Add, a, b, SPAN, U64), None);
}

#[test]
fn runtime_unop_returns_none() {
  let mut h = Harness::new();
  let a = h.runtime();

  assert_eq!(h.fold().fold_unop(UnOp::Neg, a, SPAN, U64), None);
}
