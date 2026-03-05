//! # The Circle Curve.

use crate::easing::Curve;

use libm::sqrtf;

/// ### The [`InCircle`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::{Curve, Easing};
///
/// let p = Easing::InCircle.y(1.0);
/// ```
#[derive(Debug)]
pub struct InCircle;

impl Curve for InCircle {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    1.0 - sqrtf(1.0 - p * p)
  }
}

#[test]
fn test_in_circle() {
  assert_eq!(InCircle.y(0.5), 0.13397461);
}

/// ### The [`OutCircle`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::{Curve, Easing};
///
/// let p = Easing::OutCircle.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutCircle;

impl Curve for OutCircle {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    sqrtf(1.0 - m * m)
  }
}

#[test]
fn test_out_circle() {
  assert_eq!(OutCircle.y(0.5), 0.8660254);
}

/// ### The [`InOutCircle`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::{Curve, Easing};
///
/// let p = Easing::InOutCircle.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutCircle;

impl Curve for InOutCircle {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return (1.0 - sqrtf(1.0 - t * t)) * 0.5;
    }

    (sqrtf(1.0 - 4.0 * m * m) + 1.0) * 0.5
  }
}

#[test]
fn test_in_out_circle() {
  assert_eq!(InOutCircle.y(0.5), 0.5);
}
