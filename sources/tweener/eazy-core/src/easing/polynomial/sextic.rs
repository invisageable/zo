//! # The Sextic Curve.
//!
//! An algebraic curve of degree six.
//!
//! #### formula.
//!
//! `p^6`

use crate::easing::Curve;

/// ### The [`InSextic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::sextic::InSextic;
///
/// let p = InSextic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InSextic;

impl Curve for InSextic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p * p * p
  }
}

#[test]
fn test_in_sextic() {
  let p = InSextic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutSextic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::sextic::OutSextic;
///
/// let p = OutSextic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutSextic;

impl Curve for OutSextic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 - m * m * m * m * m * m
  }
}

#[test]
fn test_out_sextic() {
  let p = OutSextic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### [`InOutSextic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::sextic::InOutSextic;
///
/// let p = InOutSextic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutSextic;

impl Curve for InOutSextic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t * t * t;
    }

    1.0 - m * m * m * m * m * m * 32.0
  }
}

#[test]
fn test_in_out_sextic() {
  let p = InOutSextic.y(1.0);

  assert_eq!(p, 1.0);
}
