//! # The Quadratic Curve.
//!
//! An algebraic curve of degree two.
//!
//! #### formula.
//!
//! `p^2`

use crate::easing::Curve;

/// ### The [`InQuadratic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::{Curve, Easing};
///
/// let p = Easing::InQuadratic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InQuadratic;

impl Curve for InQuadratic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p
  }
}

#[test]
fn test_in_quadratic() {
  assert_eq!(InQuadratic.y(0.0), 0.0);
  assert_eq!(InQuadratic.y(0.5), 0.25);
  assert_eq!(InQuadratic.y(1.0), 1.0);
}

/// ### The [`OutQuadratic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quadratic::OutQuadratic;
///
/// let p = OutQuadratic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutQuadratic;

impl Curve for OutQuadratic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 - m * m
  }
}

#[test]
fn test_out_quadratic() {
  assert_eq!(OutQuadratic.y(0.0), 0.0);
  assert_eq!(OutQuadratic.y(0.5), 0.75);
  assert_eq!(OutQuadratic.y(1.0), 1.0);
}

/// ### The [`InOutQuadratic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quadratic::InOutQuadratic;
///
/// let p = InOutQuadratic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutQuadratic;

impl Curve for InOutQuadratic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 { p * t } else { 1.0 - m * m * 2.0 }
  }
}

#[test]
fn test_in_out_quadratic() {
  assert_eq!(InOutQuadratic.y(0.0), 0.0);
  assert_eq!(InOutQuadratic.y(0.25), 0.125);
  assert_eq!(InOutQuadratic.y(0.5), 0.5);
  assert_eq!(InOutQuadratic.y(1.0), 1.0);
}
