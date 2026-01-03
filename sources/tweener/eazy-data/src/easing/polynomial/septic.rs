//! # The Septic Curve.
//!
//! An algebric curve of degree seven.
//!
//! #### formula.
//!
//! `p^7`

use crate::easing::Curve;

/// ### The [`InSeptic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::septic::InSeptic;
///
/// let p = InSeptic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InSeptic;

impl Curve for InSeptic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p * p * p * p
  }
}

#[test]
fn test_in_septic() {
  let p = InSeptic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutSeptic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::septic::OutSeptic;
///
/// let p = OutSeptic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutSeptic;

impl Curve for OutSeptic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 + m * m * m * m * m * m * m
  }
}

#[test]
fn test_out_septic() {
  let p = OutSeptic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutSeptic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::septic::InOutSeptic;
///
/// let p = InOutSeptic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutSeptic;

impl Curve for InOutSeptic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t * t * t * t;
    }

    1.0 + m * m * m * m * m * m * m * 64.0
  }
}

#[test]
fn test_in_out_septic() {
  let p = InOutSeptic.y(1.0);

  assert_eq!(p, 1.0);
}
