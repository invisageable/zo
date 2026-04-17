use super::common::{Harness, SPAN};

use crate::FoldResult;

use zo_error::{Error, ErrorKind};
use zo_sir::{BinOp, UnOp};
use zo_ty::{FloatWidth, IntWidth, Ty};

const U8: Ty = Ty::Int {
  signed: false,
  width: IntWidth::U8,
};
const S8: Ty = Ty::Int {
  signed: true,
  width: IntWidth::S8,
};
const U16: Ty = Ty::Int {
  signed: false,
  width: IntWidth::U16,
};
const S16: Ty = Ty::Int {
  signed: true,
  width: IntWidth::S16,
};
const U32: Ty = Ty::Int {
  signed: false,
  width: IntWidth::U32,
};
const S32: Ty = Ty::Int {
  signed: true,
  width: IntWidth::S32,
};

fn is_overflow(result: Option<FoldResult>) -> bool {
  matches!(
    result,
    Some(FoldResult::Error(err))
      if err == Error::new(ErrorKind::IntegerOverflow, SPAN)
  )
}

// — u8 overflow: 200 + 100 = 300 > 255.

#[test]
fn u8_add_overflow() {
  let mut h = Harness::new();
  let a = h.int(200);
  let b = h.int(100);

  assert!(is_overflow(h.fold().fold_binop(BinOp::Add, a, b, SPAN, U8),));
}

// — u8 valid: 200 + 55 = 255 (max u8).

#[test]
fn u8_add_at_max() {
  let mut h = Harness::new();
  let a = h.int(200);
  let b = h.int(55);

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, a, b, SPAN, U8),
    Some(FoldResult::Int(255)),
  );
}

// — s8 overflow: 100 + 100 = 200 > 127.

#[test]
fn s8_add_overflow() {
  let mut h = Harness::new();
  let a = h.int(100);
  let b = h.int(100);

  assert!(is_overflow(h.fold().fold_binop(BinOp::Add, a, b, SPAN, S8),));
}

// — s8 valid: 50 + 77 = 127 (max s8).

#[test]
fn s8_add_at_max() {
  let mut h = Harness::new();
  let a = h.int(50);
  let b = h.int(77);

  assert_eq!(
    h.fold().fold_binop(BinOp::Add, a, b, SPAN, S8),
    Some(FoldResult::Int(127)),
  );
}

// — u16 overflow: 60000 + 10000 = 70000 > 65535.

#[test]
fn u16_add_overflow() {
  let mut h = Harness::new();
  let a = h.int(60000);
  let b = h.int(10000);

  assert!(is_overflow(h.fold().fold_binop(
    BinOp::Add,
    a,
    b,
    SPAN,
    U16
  ),));
}

// — u16 mul overflow: 300 * 300 = 90000 > 65535.

#[test]
fn u16_mul_overflow() {
  let mut h = Harness::new();
  let a = h.int(300);
  let b = h.int(300);

  assert!(is_overflow(h.fold().fold_binop(
    BinOp::Mul,
    a,
    b,
    SPAN,
    U16
  ),));
}

// — u32 overflow: 3_000_000_000 + 2_000_000_000 > u32::MAX.

#[test]
fn u32_add_overflow() {
  let mut h = Harness::new();
  let a = h.int(3_000_000_000);
  let b = h.int(2_000_000_000);

  assert!(is_overflow(h.fold().fold_binop(
    BinOp::Add,
    a,
    b,
    SPAN,
    U32
  ),));
}

// — signed comparisons: -1 < 0 for s8.

#[test]
fn s8_signed_comparison() {
  let mut h = Harness::new();
  // -1 stored as u64 two's complement.
  let a = h.int((-1i64) as u64);
  let b = h.int(0);

  assert_eq!(
    h.fold().fold_binop(BinOp::Lt, a, b, SPAN, S8),
    Some(FoldResult::Bool(true)),
  );
}

// — unsigned comparison: same bit pattern, -1 as u64 > 0.

#[test]
fn u64_unsigned_comparison() {
  let mut h = Harness::new();
  let a = h.int((-1i64) as u64); // u64::MAX
  let b = h.int(0);

  assert_eq!(
    h.fold()
      .fold_binop(BinOp::Lt, a, b, SPAN, super::common::U64),
    Some(FoldResult::Bool(false)),
  );
}

// — shift amount too large for narrow types.

#[test]
fn u8_shl_too_large() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.int(8); // shift >= 8 for u8.

  assert!(matches!(
    h.fold().fold_binop(BinOp::Shl, a, b, SPAN, U8),
    Some(FoldResult::Error(_)),
  ));
}

#[test]
fn u8_shl_boundary_7() {
  let mut h = Harness::new();
  let a = h.int(1);
  let b = h.int(7); // shift 7 for u8 is valid: 1 << 7 = 128.

  assert_eq!(
    h.fold().fold_binop(BinOp::Shl, a, b, SPAN, U8),
    Some(FoldResult::Int(128)),
  );
}

// — bitwise not masks to width.

#[test]
fn u8_bitnot() {
  let mut h = Harness::new();
  let a = h.int(0);

  assert_eq!(
    h.fold().fold_unop(UnOp::BitNot, a, SPAN, U8),
    Some(FoldResult::Int(0xFF)), // 255, not u64::MAX.
  );
}

#[test]
fn u16_bitnot() {
  let mut h = Harness::new();
  let a = h.int(0);

  assert_eq!(
    h.fold().fold_unop(UnOp::BitNot, a, SPAN, U16),
    Some(FoldResult::Int(0xFFFF)),
  );
}

// — bitwise ops mask to width.

#[test]
fn u8_bitor_masks() {
  let mut h = Harness::new();
  let a = h.int(0xFF);
  let b = h.int(0xFF);

  assert_eq!(
    h.fold().fold_binop(BinOp::BitOr, a, b, SPAN, U8),
    Some(FoldResult::Int(0xFF)),
  );
}

// — shl masks to width.

#[test]
fn u8_shl_masks() {
  let mut h = Harness::new();
  let a = h.int(0xFF);
  let b = h.int(4);

  // 0xFF << 4 = 0xFF0, masked to u8 = 0xF0.
  assert_eq!(
    h.fold().fold_binop(BinOp::Shl, a, b, SPAN, U8),
    Some(FoldResult::Int(0xF0)),
  );
}

// — signed negation with width validation.

#[test]
fn s8_neg_valid() {
  let mut h = Harness::new();
  let a = h.int(5);

  assert_eq!(
    h.fold().fold_unop(UnOp::Neg, a, SPAN, S8),
    Some(FoldResult::Int((-5i64) as u64)),
  );
}

// — s32 arithmetic.

#[test]
fn s32_mul_overflow() {
  let mut h = Harness::new();
  // 100_000 * 100_000 = 10_000_000_000 > s32::MAX (2_147_483_647).
  let a = h.int(100_000);
  let b = h.int(100_000);

  assert!(is_overflow(h.fold().fold_binop(
    BinOp::Mul,
    a,
    b,
    SPAN,
    S32
  ),));
}

// — arithmetic shift right for signed types.

#[test]
fn s16_arithmetic_shr() {
  let mut h = Harness::new();
  // -16 as u64 two's complement.
  let a = h.int((-16i64) as u64);
  let b = h.int(2);

  // Arithmetic right shift: -16 >> 2 = -4. Stored as the
  // full-width `i64` bit pattern — the signed `Shr` path
  // keeps the sign extension so the runtime's `cmp x0, #0
  // / b.ge` test correctly detects the negative. Masking
  // to width would zero-extend the high bits and the
  // itoa path would print `65532` instead of `-4`.
  assert_eq!(
    h.fold().fold_binop(BinOp::Shr, a, b, SPAN, S16),
    Some(FoldResult::Int(-4i64 as u64)),
  );
}

// — float f32 precision narrowing.

const F32: Ty = Ty::Float(FloatWidth::F32);
const F64_TY: Ty = Ty::Float(FloatWidth::F64);

#[test]
fn f32_add_narrows_precision() {
  let mut h = Harness::new();
  // 0.1 + 0.2 in f64 = 0.30000000000000004
  // in f32 = 0.3 (less precise)
  let a = h.float(0.1);
  let b = h.float(0.2);

  let result = h.fold().fold_binop(BinOp::Add, a, b, SPAN, F32);

  // Should be the f32-precision value, not f64-precision.
  let expected = (0.1f64 + 0.2f64) as f32 as f64;

  assert_eq!(result, Some(FoldResult::Float(expected)));
}

#[test]
fn f64_add_keeps_precision() {
  let mut h = Harness::new();
  let a = h.float(0.1);
  let b = h.float(0.2);

  let result = h.fold().fold_binop(BinOp::Add, a, b, SPAN, F64_TY);

  // Should be full f64-precision.
  assert_eq!(result, Some(FoldResult::Float(0.1 + 0.2)));
}

#[test]
fn f32_overflow_to_infinity() {
  let mut h = Harness::new();
  // f32::MAX ≈ 3.4e38, so 3.5e38 * 2.0 overflows f32 to infinity.
  let a = h.float(3.5e38);
  let b = h.float(2.0);

  let result = h.fold().fold_binop(BinOp::Mul, a, b, SPAN, F32);

  assert!(matches!(result, Some(FoldResult::Error(err))
    if err == zo_error::Error::new(
      zo_error::ErrorKind::FloatInfinity, SPAN
    )
  ));
}

#[test]
fn f32_within_range_ok() {
  let mut h = Harness::new();
  let a = h.float(1.0e10);
  let b = h.float(2.0);

  let result = h.fold().fold_binop(BinOp::Mul, a, b, SPAN, F32);

  let expected = (1.0e10f64 * 2.0f64) as f32 as f64;

  assert_eq!(result, Some(FoldResult::Float(expected)));
}

#[test]
fn f32_neg_validates() {
  let mut h = Harness::new();
  let a = h.float(1.5);

  let result = h.fold().fold_unop(UnOp::Neg, a, SPAN, F32);

  let expected = (-1.5f64) as f32 as f64;

  assert_eq!(result, Some(FoldResult::Float(expected)));
}

#[test]
fn f64_mul_overflow_to_infinity() {
  let mut h = Harness::new();
  let a = h.float(f64::MAX);
  let b = h.float(2.0);

  let result = h.fold().fold_binop(BinOp::Mul, a, b, SPAN, F64_TY);

  assert!(matches!(result, Some(FoldResult::Error(err))
    if err == zo_error::Error::new(
      zo_error::ErrorKind::FloatInfinity, SPAN
    )
  ));
}
