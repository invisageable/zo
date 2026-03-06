//! # The Cubic Curve.
//!
//! An algebraic curve of degree three.
//!
//! #### formula.
//!
//! `p^3`

use crate::easing::Curve;

use wide::{CmpLt, f32x8};

/// ### The [`InCubic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InCubic.y(1.0);
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
  assert_eq!(InCubic.y(0.0), 0.0);
  assert_eq!(InCubic.y(0.5), 0.125);
  assert_eq!(InCubic.y(1.0), 1.0);
}

/// ### The [`OutCubic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::OutCubic.y(1.0);
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
  assert_eq!(OutCubic.y(0.0), 0.0);
  assert_eq!(OutCubic.y(0.5), 0.875);
  assert_eq!(OutCubic.y(1.0), 1.0);
}

/// ### The [`InOutCubic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InOutCubic.y(1.0);
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

#[test]
fn test_in_out_cubic() {
  assert_eq!(InOutCubic.y(0.0), 0.0);
  assert_eq!(InOutCubic.y(0.25), 0.0625);
  assert_eq!(InOutCubic.y(0.5), 0.5);
  assert_eq!(InOutCubic.y(1.0), 1.0);
}

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
