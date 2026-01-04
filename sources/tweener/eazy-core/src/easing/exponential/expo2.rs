//! # The Exponent 2 Curve.
//!
//! #### notes.
//!
//! `Exponent2` needs clamping for `0` and `1`.
//! Use exp2f instead of powf(2.0, x) - exp2f is optimized for base 2.

use crate::easing::Curve;

use libm::exp2f;

/// ### The [`InExpo2`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::exponential::expo2::InExpo2;
///
/// let p = InExpo2.y(1.0);
/// ```
#[derive(Debug)]
pub struct InExpo2;

impl Curve for InExpo2 {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p <= 0.0 {
      return 0.0;
    }

    exp2f(10.0 * (p - 1.0))
  }
}

#[test]
fn test_in_expo2() {
  let p = InExpo2.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutExpo2`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::exponential::expo2::OutExpo2;
///
/// let p = OutExpo2.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutExpo2;

impl Curve for OutExpo2 {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p >= 1.0 {
      return 1.0;
    }

    1.0 - exp2f(-10.0 * p)
  }
}

#[test]
fn test_out_expo2() {
  let p = OutExpo2.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutExpo2`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::exponential::expo2::InOutExpo2;
///
/// let p = InOutExpo2.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutExpo2;

impl Curve for InOutExpo2 {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    if p <= 0.0 {
      return 0.0;
    }

    if p >= 1.0 {
      return 1.0;
    }

    if p < 0.5 {
      return exp2f(10.0 * (2.0 * p - 1.0) - 1.0);
    }

    1.0 - exp2f(-10.0 * (2.0 * p - 1.0) - 1.0)
  }
}

#[test]
fn test_in_out_expo2() {
  let p = InOutExpo2.y(1.0);

  assert_eq!(p, 1.0);
}
