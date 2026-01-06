//! # The Decic Curve.
//!
//! An algebraic curve of degree ten.
//!
//! #### formula.
//!
//! `p^10`

use crate::easing::Curve;

/// ### The [`InDecic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::decic::InDecic;
///
/// let p = InDecic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InDecic;

impl Curve for InDecic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p * p * p * p * p * p * p
  }
}

#[test]
fn test_in_decic() {
  assert_eq!(InDecic.y(1.0), 1.0);
}

/// ### The [`OutDecic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::decic::OutDecic;
///
/// let p = OutDecic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutDecic;

impl Curve for OutDecic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 - m * m * m * m * m * m * m * m * m * m
  }
}

#[test]
fn test_out_decic() {
  assert_eq!(OutDecic.y(1.0), 1.0);
}

/// ### The [`InOutDecic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::decic::InOutDecic;
///
/// let p = InOutDecic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutDecic;

impl Curve for InOutDecic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t * t * t * t * t * t;
    }

    1.0 - m * m * m * m * m * m * m * m * m * m * 512.0
  }
}

#[test]
fn test_in_out_decic() {
  assert_eq!(InOutDecic.y(1.0), 1.0);
}
