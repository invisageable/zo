//! # The Square Root Curve.
//!
//! An algebraic curve of degree 1/2.

use crate::easing::Curve;
use crate::math::sqrtf;

/// ### The [`InSqrt`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InSqrt.y(0.25);
/// ```
#[derive(Debug)]
pub struct InSqrt;

impl Curve for InSqrt {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    1.0 - OutSqrt.y(1.0 - p)
  }
}

#[test]
fn test_in_sqrt() {
  assert_eq!(InSqrt.y(0.0), 0.0);
  assert_eq!(InSqrt.y(1.0), 1.0);

  let p = InSqrt.y(0.5);

  assert!((p - 0.29289).abs() < 1e-4, "InSqrt(0.5) = {p}");
}

/// ### The [`InOutSqrt`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InOutSqrt.y(0.25);
/// ```
#[derive(Debug)]
pub struct InOutSqrt;

impl Curve for InOutSqrt {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let t = p * 2.0;

    if t < 1.0 {
      return 0.5 - 0.5 * OutSqrt.y(1.0 - t);
    }

    0.5 + 0.5 * OutSqrt.y(t - 1.0)
  }
}

#[test]
fn test_in_out_sqrt() {
  assert_eq!(InOutSqrt.y(0.0), 0.0);
  assert_eq!(InOutSqrt.y(1.0), 1.0);

  let p = InOutSqrt.y(0.5);

  assert!((p - 0.5).abs() < 1e-4, "InOutSqrt(0.5) = {p}");
}

/// ### The [`OutSqrt`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::OutSqrt.y(0.25);
/// ```
#[derive(Debug)]
pub struct OutSqrt;

impl Curve for OutSqrt {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    sqrtf(p)
  }
}

#[test]
fn test_out_sqrt() {
  assert_eq!(OutSqrt.y(0.0), 0.0);
  assert_eq!(OutSqrt.y(1.0), 1.0);

  let p = OutSqrt.y(0.5);

  assert!(
    (p - core::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6,
    "OutSqrt(0.5) = {p}"
  );
}
