//! # The Quartic Curve.
//!
//! An algebric curve of degree four.
//!
//! #### formula.
//!
//! `p^4`

use crate::easing::Curve;

/// ### The [`InQuartic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quartic::InQuartic;
///
/// let p = InQuartic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InQuartic;

impl Curve for InQuartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p
  }
}

#[test]
fn test_in_quartic() {
  let p = InQuartic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutQuartic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quartic::OutQuartic;
///
/// let p = OutQuartic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutQuartic;

impl Curve for OutQuartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 - m * m * m * m
  }
}

#[test]
fn test_out_quartic() {
  let p = OutQuartic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutQuartic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::quartic::InOutQuartic;
///
/// let p = InOutQuartic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutQuartic;

impl Curve for InOutQuartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t;
    }

    1.0 - m * m * m * m * 8.0
  }
}

#[test]
fn test_in_out_quartic() {
  let p = InOutQuartic.y(1.0);

  assert_eq!(p, 1.0);
}
