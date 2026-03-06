//! # The Nonic Curve.
//!
//! An algebraic curve of degree nine.
//!
//! #### formula.
//!
//! `p^9`

use crate::easing::Curve;

/// ### The [`InNonic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InNonic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InNonic;

impl Curve for InNonic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p * p * p * p * p * p
  }
}

#[test]
fn test_in_nonic() {
  assert_eq!(InNonic.y(0.0), 0.0);
  assert_eq!(InNonic.y(0.5), 0.001953125);
  assert_eq!(InNonic.y(1.0), 1.0);
}

/// ### The [`OutNonic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::OutNonic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutNonic;

impl Curve for OutNonic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 + m * m * m * m * m * m * m * m * m
  }
}

#[test]
fn test_out_nonic() {
  assert_eq!(OutNonic.y(0.0), 0.0);
  assert_eq!(OutNonic.y(0.5), 0.998_046_9);
  assert_eq!(OutNonic.y(1.0), 1.0);
}

/// ### The [`InOutNonic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InOutNonic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutNonic;

impl Curve for InOutNonic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t * t * t * t * t * t;
    }

    1.0 + m * m * m * m * m * m * m * m * m * 256.0
  }
}

#[test]
fn test_in_out_nonic() {
  assert_eq!(InOutNonic.y(0.0), 0.0);
  assert_eq!(InOutNonic.y(0.25), 0.0009765625);
  assert_eq!(InOutNonic.y(0.5), 0.5);
  assert_eq!(InOutNonic.y(1.0), 1.0);
}
