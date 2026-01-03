//! The Piecewize Polynomial Curve.

use crate::easing::Curve;

use libm::powf;

/// The [`InPiecewizePolynomial`] Curve.
#[derive(Debug)]
pub struct InPiecewizePolynomial;

impl Curve for InPiecewizePolynomial {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p < 0.5 {
      return 4.0 * powf(p, 3.0);
    }

    1.0 - powf(-2.0 * p + 2.0, 2.0) / 2.0
  }
}

#[test]
fn test_in_piecewize_polynomial() {
  let p = InPiecewizePolynomial.y(1.0);

  assert_eq!(p, 1.0)
}

/// The [`OutPiecewizePolynomial`] Curve.
#[derive(Debug)]
pub struct OutPiecewizePolynomial;

impl Curve for OutPiecewizePolynomial {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p < 0.5 {
      return powf(2.0 * p, 2.0) / 2.0;
    }

    1.0 - powf(-2.0 * p + 2.0, 3.0) / 2.0
  }
}

#[test]
fn test_out_piecewize_polynomial() {
  let p = OutPiecewizePolynomial.y(1.0);

  assert_eq!(p, 1.0)
}
