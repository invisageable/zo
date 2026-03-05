//! # The Exponent E Curve.
//!
//! An algebraic curve of degree infinite.
//!
//! #### formula.
//!
//! `e^x`

use crate::easing::Curve;

use libm::powf;

/// ### The [`InExpoE`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::exponential::expoe::InExpoE;
///
/// let p = InExpoE.y(0.4);
/// ```
#[derive(Debug)]
pub struct InExpoE;

impl Curve for InExpoE {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p <= 0.0 {
      return 0.0;
    }

    powf(core::f32::consts::E, -10.0 * (1.0 - p))
  }
}

#[test]
fn test_in_expoe() {
  assert_eq!(InExpoE.y(0.0), 0.0);
  assert_eq!(InExpoE.y(1.0), 1.0);
  let p = InExpoE.y(0.5);
  assert!((p - 0.006738).abs() < 1e-4, "InExpoE(0.5) = {p}");
}

/// ### The [`OutExpoE`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::exponential::expoe::OutExpoE;
///
/// let p = OutExpoE.y(0.4);
/// ```
#[derive(Debug)]
pub struct OutExpoE;

impl Curve for OutExpoE {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    1.0 - InExpoE.y(1.0 - p)
  }
}

#[test]
fn test_out_expoe() {
  assert_eq!(OutExpoE.y(0.0), 0.0);
  assert_eq!(OutExpoE.y(1.0), 1.0);
  let p = OutExpoE.y(0.5);
  assert!((p - 0.99326).abs() < 1e-4, "OutExpoE(0.5) = {p}");
}

/// ### The [`InOutExpoE`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::exponential::expoe::InOutExpoE;
///
/// let p = InOutExpoE.y(0.4);
/// ```
#[derive(Debug)]
pub struct InOutExpoE;

impl Curve for InOutExpoE {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let t = p * 2.0;

    if t < 1.0 {
      return 0.5 - 0.5 * OutExpoE.y(1.0 - t);
    }

    0.5 + 0.5 * OutExpoE.y(t - 1.0)
  }
}

#[test]
fn test_in_out_expoe() {
  assert_eq!(InOutExpoE.y(0.0), 0.0);
  assert_eq!(InOutExpoE.y(1.0), 1.0);
  let p = InOutExpoE.y(0.5);
  assert!((p - 0.5).abs() < 1e-4, "InOutExpoE(0.5) = {p}");
}
