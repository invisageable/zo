//! The Cubic Interpolating Rational Curve.

use crate::easing::Curve;

use libm::powf;

/// The [`InRationalCubic`] Curve.
#[derive(Debug)]
pub struct InRationalCubic;

impl Curve for InRationalCubic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p / (3.0 * p * p - 3.0 * p + 1.0)
  }
}

#[test]
fn test_in_rational_cubic() {
  let p = InRationalCubic.y(0.2);

  assert_eq!(p, 0.015384616);
}

/// The [`OutRationalCubic`] Curve.
#[derive(Debug)]
pub struct OutRationalCubic;

impl Curve for OutRationalCubic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let a = powf(p, 1.0 / 3.0);
    let b = powf(1.0 - p, 1.0 / 3.0);

    a / (a + b)
  }
}

#[test]
fn test_out_rational_cubic() {
  let p = OutRationalCubic.y(0.42);

  assert_eq!(p, 0.47312814);
}
