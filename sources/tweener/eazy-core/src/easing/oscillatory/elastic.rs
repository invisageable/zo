//! # The Elastic Curve.
//!
//! Follows the easings.net formula with boundary clamping and FMA.
//!
//! #### formula.
//!
//! `-(2^(10t-10)) * sin((10t - 10.75) * C4)`
//!
//! Where `C4 = (2π)/3` and `C5 = (2π)/4.5`.

use crate::easing::Curve;
use crate::math::{exp2f, sinf};

use core::f32::consts::PI;

const C4: f32 = (2.0 * PI) / 3.0;
const C5: f32 = (2.0 * PI) / 4.5;

/// ### The [`InElastic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::{Curve, Easing};
///
/// let p = Easing::InElastic.y(0.5);
/// ```
#[derive(Debug)]
pub struct InElastic;

impl Curve for InElastic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p <= 0.0 {
      return 0.0;
    }

    if 1.0 <= p {
      return 1.0;
    }

    -(exp2f(10.0f32.mul_add(p, -10.0))) * sinf(p.mul_add(10.0, -10.75) * C4)
  }
}

#[test]
fn test_in_elastic() {
  assert_eq!(InElastic.y(0.0), 0.0);
  assert_eq!(InElastic.y(1.0), 1.0);

  let p = InElastic.y(0.5);

  assert!((p - -0.015625).abs() < 1e-4, "InElastic(0.5) = {p}");
}

/// ### The [`OutElastic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::{Curve, Easing};
///
/// let p = Easing::OutElastic.y(0.5);
/// ```
#[derive(Debug)]
pub struct OutElastic;

impl Curve for OutElastic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p <= 0.0 {
      return 0.0;
    }

    if 1.0 <= p {
      return 1.0;
    }

    exp2f((-10.0) * p).mul_add(sinf(p.mul_add(10.0, -0.75) * C4), 1.0)
  }
}

#[test]
fn test_out_elastic() {
  assert_eq!(OutElastic.y(0.0), 0.0);
  assert_eq!(OutElastic.y(1.0), 1.0);

  let p = OutElastic.y(0.5);

  assert!((p - 1.015625).abs() < 1e-4, "OutElastic(0.5) = {p}");
}

/// ### The [`InOutElastic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::{Curve, Easing};
///
/// let p = Easing::InOutElastic.y(0.5);
/// ```
#[derive(Debug)]
pub struct InOutElastic;

impl Curve for InOutElastic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p <= 0.0 {
      return 0.0;
    }

    if 1.0 <= p {
      return 1.0;
    }

    if p < 0.5 {
      return -(exp2f(20.0f32.mul_add(p, -10.0))
        * sinf(20.0f32.mul_add(p, -11.125) * C5))
        / 2.0;
    }

    (exp2f((-20.0f32).mul_add(p, 10.0))
      * sinf(20.0f32.mul_add(p, -11.125) * C5))
      / 2.0
      + 1.0
  }
}

#[test]
fn test_in_out_elastic() {
  assert_eq!(InOutElastic.y(0.0), 0.0);
  assert_eq!(InOutElastic.y(1.0), 1.0);

  let p = InOutElastic.y(0.5);

  assert!((p - 0.5).abs() < 1e-4, "InOutElastic(0.5) = {p}");
}
