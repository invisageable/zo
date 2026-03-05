//! # The Sine Curve.

use crate::easing::Curve;

use libm::{cosf, sinf};

use core::f32::consts::PI;

/// ### The [`InSine`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::{Curve, Easing};
///
/// let p = Easing::InSine.y(0.5);
///
/// assert_eq!(p, 0.29289323);
/// ```
#[derive(Debug)]
pub struct InSine;

impl Curve for InSine {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    1.0 - cosf(p * PI * 0.5)
  }
}

#[test]
fn test_in_sine() {
  assert_eq!(InSine.y(0.5), 0.29289323);
}

/// ### The [`OutSine`] Easing Function.
///
/// Also see [`Curve`].
///
/// #### examples.
///
/// ```rust
/// use eazy::{Curve, Easing};
///
/// let p = Easing::OutSine.y(0.1264);
///
/// assert_eq!(p, 0.1972467);
/// ```
#[derive(Debug)]
pub struct OutSine;

impl Curve for OutSine {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    sinf(p * PI * 0.5)
  }
}

#[test]
fn test_out_sine() {
  assert_eq!(OutSine.y(0.1264), 0.1972467);
}

/// ### The [`InOutSine`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::{Curve, Easing};
///
/// let p = Easing::InOutSine.y(0.248608);
///
/// assert_eq!(p, 0.14490387);
/// ```
#[derive(Debug)]
pub struct InOutSine;

impl Curve for InOutSine {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    0.5 * (1.0 - cosf(p * PI))
  }
}

#[test]
fn test_in_out_sine() {
  assert_eq!(InOutSine.y(0.248608), 0.14490387);
}
