//! # The Quintic Curve.
//!
//! An algebric curve of degree five.
//!
//! #### formula.
//!
//! `p^5`

use crate::easing::Curve;

/// ### The [`InQuintic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quintic::InQuintic;
///
/// let p = InQuintic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InQuintic;

impl Curve for InQuintic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p * p
  }
}

#[test]
fn test_in_quintic() {
  let p = InQuintic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutQuintic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quintic::OutQuintic;
///
/// let p = OutQuintic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutQuintic;

impl Curve for OutQuintic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 + m * m * m * m * m
  }
}

#[test]
fn test_out_quintic() {
  let p = OutQuintic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutQuintic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quintic::InOutQuintic;
///
/// let p = InOutQuintic.y(1.0);
/// ```   
#[derive(Debug)]
pub struct InOutQuintic;

impl Curve for InOutQuintic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t * t;
    }

    1.0 + m * m * m * m * m * 16.0
  }
}

#[test]
fn test_in_out_quintic() {
  let p = InOutQuintic.y(1.0);

  assert_eq!(p, 1.0);
}
