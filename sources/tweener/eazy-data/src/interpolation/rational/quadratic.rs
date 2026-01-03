//! The Quadratic Interpolating Rational Curve.

use crate::easing::Curve;

use libm::sqrtf;

/// The [`InRationalQuadratic`] Curve.
#[derive(Debug)]
pub struct InRationalQuadratic;

impl Curve for InRationalQuadratic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p / (2.0 * p * p - 2.0 * p + 1.0)
  }
}

#[test]
fn test_in_rational_quadratic() {
  let p = InRationalQuadratic.y(1.0);

  assert_eq!(p, 1.0);
}

/// The [`OutRationalQuadratic`] Curve.
#[derive(Debug)]
pub struct OutRationalQuadratic;

impl Curve for OutRationalQuadratic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    (p - sqrtf(p * (1.0 - p))) / (2.0 * p - 1.0)
  }
}

#[test]
fn test_out_rational_quadratic() {
  let p = OutRationalQuadratic.y(1.0);

  assert_eq!(p, 1.0);
}
