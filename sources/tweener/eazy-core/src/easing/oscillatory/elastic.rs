//! # The Elastic Curve.
//!
//! Use exp2f instead of powf(2.0, x) - exp2f is optimized for base 2.

use crate::easing::Curve;

use libm::{exp2f, sinf};

/// ### The [`InElastic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::standard::elastic::InElastic;
///
/// let p = InElastic.y(0.2222);
/// ```
#[derive(Debug)]
pub struct InElastic;

impl Curve for InElastic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    -exp2f(10.0 * m) * sinf((m * 40.0 - 3.0) * core::f32::consts::PI / 6.0)
  }
}

#[test]
fn test_in_elastic() {
  let p = InElastic.y(0.2222);

  assert_eq!(p, -0.0038053591);
}

/// ### The [`OutElastic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::standard::elastic::OutElastic;
///
/// let p = OutElastic.y(1.0);
///
/// assert_eq!(p, 1.0004883);
/// ```
#[derive(Debug)]
pub struct OutElastic;

impl Curve for OutElastic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    1.0
      + (exp2f(-10.0 * p)
        * sinf((-p * 40.0 - 3.0) * core::f32::consts::PI / 6.0))
  }
}

#[test]
fn test_out_elastic() {
  let p = OutElastic.y(0.5345);

  assert_eq!(p, 0.9951369);
}

/// ### The [`InOutElastic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::standard::elastic::InOutElastic;
///
/// let p = InOutElastic.y(0.5345);
/// ```
#[derive(Debug)]
pub struct InOutElastic;

impl Curve for InOutElastic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let s = 2.0 * p - 1.0; // remap: [0,0.5] -> [-1,0]
    let k = (80.0 * s - 9.0) * core::f32::consts::PI / 18.0; // and [0.5,1] -> [0,+1]

    if s < 0.0 {
      return -0.5 * exp2f(10.0 * s) * sinf(k);
    }

    1.0 + 0.5 * exp2f(-10.0 * s) * sinf(k)
  }
}

#[test]
fn test_in_out_elastic() {
  let p = InOutElastic.y(0.5345);

  assert_eq!(p, 0.82312053);
}
