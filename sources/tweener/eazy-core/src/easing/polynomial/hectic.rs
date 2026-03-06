//! # The Hectic Curve.
//!
//! An algebraic curve of degree one hundred.
//!
//! #### Formula.
//!
//! `p^100`

use crate::easing::Curve;

/// ### The [`InHectic`] Easing Function.
///
/// #### Examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InHectic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InHectic;

impl Curve for InHectic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p.powi(100)
  }
}

#[test]
fn test_in_hectic() {
  assert_eq!(InHectic.y(0.0), 0.0);
  assert!(InHectic.y(0.5) < 0.001);
  assert_eq!(InHectic.y(1.0), 1.0);
}

/// ### The [`OutHectic`] Easing Function.
///
/// #### Examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::OutHectic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutHectic;

impl Curve for OutHectic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 - m.powi(100)
  }
}

#[test]
fn test_out_hectic() {
  assert_eq!(OutHectic.y(0.0), 0.0);
  assert!(OutHectic.y(0.5) > 0.999);
  assert_eq!(OutHectic.y(1.0), 1.0);
}

/// ### The [`InOutHectic`] Easing Function.
///
/// #### Examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InOutHectic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutHectic;

impl Curve for InOutHectic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return 0.5 * t.powi(100);
    }

    1.0 - 0.5 * (2.0 * m).powi(100)
  }
}

#[test]
fn test_in_out_hectic() {
  assert_eq!(InOutHectic.y(0.0), 0.0);
  assert_eq!(InOutHectic.y(0.5), 0.5);
  assert_eq!(InOutHectic.y(1.0), 1.0);
}
