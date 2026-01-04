//! The Constant Interpolating Polynomial Curve.

use crate::easing::Curve;

/// The [`None`] Curve.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::none::None;
///
/// let p = None.y(1.0);
/// ```
#[derive(Debug)]
pub struct None;

impl Curve for None {
  #[inline(always)]
  fn y(&self, _p: f32) -> f32 {
    1.0
  }
}

#[test]
fn test_none() {
  let p = None.y(0.8);

  assert_eq!(p, 1.0);
}
