//! The Smoothstep Interpolating Polynomial Curve.
//!
//! Smoothstep is a polynomial S-curve defined as `3t² - 2t³`.
//! This module provides In, Out, and InOut variants derived mathematically:
//!
//! - **InOut** (standard): `3t² - 2t³` - symmetric S-curve
//! - **In**: First half of InOut stretched to [0,1]: `t² * (3 - t) / 2`
//! - **Out**: Mirror of In: `t * (3 - t²) / 2`

use crate::easing::Curve;

/// The [`InSmooth`] Curve (ease-in).
///
/// Derived from smoothstep InOut by taking the first half and stretching:
/// `In(t) = 2 * InOut(t/2) = t² * (3 - t) / 2`
///
/// Starts slow, accelerates toward the end.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::smoothstep::InSmooth;
///
/// let p = InSmooth.y(0.5);
/// assert!((p - 0.3125).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct InSmooth;

impl Curve for InSmooth {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - p) * 0.5
  }
}

#[test]
fn test_in_smooth() {
  // In(0) = 0
  assert_eq!(InSmooth.y(0.0), 0.0);
  // In(1) = 1
  assert_eq!(InSmooth.y(1.0), 1.0);
  // In(0.5) = 0.3125 (ease-in: slower at start)
  assert!((InSmooth.y(0.5) - 0.3125).abs() < 0.0001);
}

/// The [`OutSmooth`] Curve (ease-out).
///
/// Mirror of InSmooth: `Out(t) = 1 - In(1-t) = t * (3 - t²) / 2`
///
/// Starts fast, decelerates toward the end.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::smoothstep::OutSmooth;
///
/// let p = OutSmooth.y(0.5);
/// assert!((p - 0.6875).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct OutSmooth;

impl Curve for OutSmooth {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * (3.0 - p * p) * 0.5
  }
}

#[test]
fn test_out_smooth() {
  // Out(0) = 0
  assert_eq!(OutSmooth.y(0.0), 0.0);
  // Out(1) = 1
  assert_eq!(OutSmooth.y(1.0), 1.0);
  // Out(0.5) = 0.6875 (ease-out: faster at start)
  assert!((OutSmooth.y(0.5) - 0.6875).abs() < 0.0001);
}

/// The [`InOutSmooth`] Curve (standard smoothstep).
///
/// The classic smoothstep S-curve: `3t² - 2t³`
///
/// Starts slow, speeds up in the middle, slows down at the end.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::smoothstep::InOutSmooth;
///
/// let p = InOutSmooth.y(0.5);
/// assert_eq!(p, 0.5);
/// ```
#[derive(Debug)]
pub struct InOutSmooth;

impl Curve for InOutSmooth {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
  }
}

#[test]
fn test_in_out_smooth() {
  // InOut(0) = 0
  assert_eq!(InOutSmooth.y(0.0), 0.0);
  // InOut(0.5) = 0.5 (symmetric)
  assert_eq!(InOutSmooth.y(0.5), 0.5);
  // InOut(1) = 1
  assert_eq!(InOutSmooth.y(1.0), 1.0);
}

/// The Non-linear Interpolation.
///
/// Interpolates smoothly between `min` and `max`. It will accelerated from the
/// start and deccelerated toward the end with a cubic easing.
///
/// #### params.
///
/// |      |                            |
/// |:-----|:---------------------------|
/// | `p`  | The progress.              |
/// | `x0` | The `min` start value.     |
/// | `x1` | The `max` end value.       |
///
/// #### returns.
///
/// `f32` — The interpolated result between the two float values.
///
/// #### examples.
///
/// ```
/// use eazy::interpolation::polynomial::smoothstep::smoothstep;
///
/// let p = smoothstep(0.25, 0.0, 1.0);
///
/// assert_eq!(p, 0.15625);
/// ```
#[inline(always)]
pub fn smoothstep(p: f32, x0: f32, x1: f32) -> f32 {
  let mut p = (p - x0) / (x1 - x0);

  p = p.clamp(0.0, 1.0);
  p * p * (3.0 - 2.0 * p)
}
