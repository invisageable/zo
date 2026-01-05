//! # The Octic Curve.
//!
//! An algebraic curve of degree eight.
//!
//! #### formula.
//!
//! `p^8`

use crate::easing::Curve;

/// ### The [`InOctic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::octic::InOctic;
///
/// let p = InOctic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOctic;

impl Curve for InOctic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p * p * p * p * p
  }
}

#[test]
fn test_in_octic() {
  let p = InOctic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutOctic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::octic::OutOctic;
///
/// let p = OutOctic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutOctic;

impl Curve for OutOctic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 - m * m * m * m * m * m * m * m
  }
}

#[test]
fn test_out_octic() {
  let p = OutOctic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutOctic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::octic::InOutOctic;
///
/// let p = InOutOctic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutOctic;

impl Curve for InOutOctic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t * t * t * t * t;
    }

    1.0 - m * m * m * m * m * m * m * m * 128.0
  }
}

#[test]
fn test_in_out_octic() {
  let p = InOutOctic.y(1.0);

  assert_eq!(p, 1.0);
}
