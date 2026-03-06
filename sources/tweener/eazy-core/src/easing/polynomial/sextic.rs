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
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InSextic.y(1.0);
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
  assert_eq!(InSextic.y(0.0), 0.0);
  assert_eq!(InSextic.y(0.5), 0.015625);
  assert_eq!(InSextic.y(1.0), 1.0);
}

/// ### The [`OutSextic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::OutSextic.y(1.0);
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
  assert_eq!(OutSextic.y(0.0), 0.0);
  assert_eq!(OutSextic.y(0.5), 0.984375);
  assert_eq!(OutSextic.y(1.0), 1.0);
}

/// ### [`InOutSextic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InOutSextic.y(1.0);
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
  assert_eq!(InOutSextic.y(0.0), 0.0);
  assert_eq!(InOutSextic.y(0.25), 0.0078125);
  assert_eq!(InOutSextic.y(0.5), 0.5);
  assert_eq!(InOutSextic.y(1.0), 1.0);
}
