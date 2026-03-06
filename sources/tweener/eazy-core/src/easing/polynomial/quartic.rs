//! # The Quartic Curve.
//!
//! An algebraic curve of degree four.
//!
//! #### formula.
//!
//! `p^4`

use crate::easing::Curve;

/// ### The [`InQuartic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InQuartic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InQuartic;

impl Curve for InQuartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * p * p
  }
}

#[test]
fn test_in_quartic() {
  assert_eq!(InQuartic.y(0.0), 0.0);
  assert_eq!(InQuartic.y(0.5), 0.0625);
  assert_eq!(InQuartic.y(1.0), 1.0);
}

/// ### The [`OutQuartic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::OutQuartic.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutQuartic;

impl Curve for OutQuartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;

    1.0 - m * m * m * m
  }
}

#[test]
fn test_out_quartic() {
  assert_eq!(OutQuartic.y(0.0), 0.0);
  assert_eq!(OutQuartic.y(0.5), 0.9375);
  assert_eq!(OutQuartic.y(1.0), 1.0);
}

/// ### The [`InOutQuartic`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InOutQuartic.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutQuartic;

impl Curve for InOutQuartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;

    if t < 1.0 {
      return p * t * t * t;
    }

    1.0 - m * m * m * m * 8.0
  }
}

#[test]
fn test_in_out_quartic() {
  assert_eq!(InOutQuartic.y(0.0), 0.0);
  assert_eq!(InOutQuartic.y(0.25), 0.03125);
  assert_eq!(InOutQuartic.y(0.5), 0.5);
  assert_eq!(InOutQuartic.y(1.0), 1.0);
}
