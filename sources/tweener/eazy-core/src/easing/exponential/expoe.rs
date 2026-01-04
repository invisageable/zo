//! # The Exponent E Curve.
//!
//! An algebric curve of degree infinite.
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
  let p = InExpoE.y(1.0);

  assert_eq!(p, 1.0);
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
  let p = OutExpoE.y(1.0);

  assert_eq!(p, 1.0);
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
  let p = InOutExpoE.y(1.0);

  assert_eq!(p, 1.0);
}
