//! # The Cubic Curve.
//!
//! An algebric curve of degree three.
//!
//! #### formula.
//!
//! `p^3`

use wide::f32x8;

use crate::easing::Curve;

/// ### The [`InCubic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::cubic::InCubic;
///
/// let p = InCubic.y(1.0);
/// ```
/// `f(t) = 2.0^(10.0 * (t - 1.0))`
#[derive(Debug)]
pub struct InCubic;

impl Curve for InCubic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p
  }
}

#[test]
fn test_in_cubic() {
  let p = InCubic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutCubic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::cubic::OutCubic;
///
/// let p = OutCubic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutCubic;

impl Curve for OutCubic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 + m * m * m
  }
}

#[test]
fn test_out_cubic() {
  let p = OutCubic.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutCubic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::polynomial::cubic::InOutCubic;
///
/// let p = InOutCubic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutCubic;

impl Curve for InOutCubic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      p * t * t
    } else {
      1.0 + m * m * m * 4.0
    }
  }
}

use wide::CmpLt;

pub fn in_out_cubic_simd(p: f32x8) -> f32x8 {
  let one = f32x8::splat(1.0);
  let two = f32x8::splat(2.0);
  let four = f32x8::splat(4.0);

  let m = p - one;
  let t = p * two;

  let t_branch = p * t * t;
  let m_branch = one + m * m * m * four;

  let mask = t.simd_lt(one);

  mask.blend(t_branch, m_branch)
}

#[test]
fn test_in_out_cubic() {
  let p = InOutCubic.y(1.0);

  assert_eq!(p, 1.0);
}
