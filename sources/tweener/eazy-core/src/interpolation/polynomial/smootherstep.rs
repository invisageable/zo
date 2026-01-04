//! The Smootherstep Interpolating Polynomial Curve.
//!
//! Smootherstep is Ken Perlin's improved S-curve: `6t⁵ - 15t⁴ + 10t³`.
//! It has zero first AND second derivatives at t=0 and t=1.
//! This module provides In, Out, and InOut variants derived mathematically:
//!
//! - **InOut** (standard): `6t⁵ - 15t⁴ + 10t³` - symmetric S-curve
//! - **In**: First half of InOut stretched: `t³ * (3t² - 15t + 20) / 8`
//! - **Out**: Mirror of In: `t * (3t⁴ - 10t² + 15) / 8`

use crate::easing::Curve;

/// The [`InSmoother`] Curve (ease-in).
///
/// Derived from smootherstep InOut by taking the first half and stretching:
/// `In(t) = 2 * InOut(t/2) = t³ * (3t² - 15t + 20) / 8`
///
/// Starts slow, accelerates toward the end.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::smootherstep::InSmoother;
///
/// let p = InSmoother.y(0.5);
/// assert!((p - 0.20703125).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct InSmoother;

impl Curve for InSmoother {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    // t³ * (3t² - 15t + 20) / 8
    let p2 = p * p;
    let p3 = p2 * p;
    p3 * (3.0 * p2 - 15.0 * p + 20.0) * 0.125
  }
}

#[test]
fn test_in_smoother() {
  // In(0) = 0
  assert_eq!(InSmoother.y(0.0), 0.0);
  // In(1) = 1
  assert_eq!(InSmoother.y(1.0), 1.0);
  // In(0.5) ≈ 0.207 (ease-in: slower at start)
  assert!((InSmoother.y(0.5) - 0.20703125).abs() < 0.0001);
}

/// The [`OutSmoother`] Curve (ease-out).
///
/// Mirror of InSmoother: `Out(t) = 1 - In(1-t) = t * (3t⁴ - 10t² + 15) / 8`
///
/// Starts fast, decelerates toward the end.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::smootherstep::OutSmoother;
///
/// let p = OutSmoother.y(0.5);
/// assert!((p - 0.79296875).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct OutSmoother;

impl Curve for OutSmoother {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    // t * (3t⁴ - 10t² + 15) / 8
    let p2 = p * p;
    let p4 = p2 * p2;
    p * (3.0 * p4 - 10.0 * p2 + 15.0) * 0.125
  }
}

#[test]
fn test_out_smoother() {
  // Out(0) = 0
  assert_eq!(OutSmoother.y(0.0), 0.0);
  // Out(1) = 1
  assert_eq!(OutSmoother.y(1.0), 1.0);
  // Out(0.5) ≈ 0.793 (ease-out: faster at start)
  assert!((OutSmoother.y(0.5) - 0.79296875).abs() < 0.0001);
}

/// The [`InOutSmoother`] Curve (standard smootherstep).
///
/// Ken Perlin's smootherstep: `6t⁵ - 15t⁴ + 10t³`
///
/// Has zero first AND second derivatives at endpoints for extra smoothness.
/// Starts slow, speeds up in the middle, slows down at the end.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::smootherstep::InOutSmoother;
///
/// let p = InOutSmoother.y(0.5);
/// assert_eq!(p, 0.5);
/// ```
#[derive(Debug)]
pub struct InOutSmoother;

impl Curve for InOutSmoother {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    // 6t⁵ - 15t⁴ + 10t³ = t³ * (6t² - 15t + 10)
    let p2 = p * p;
    let p3 = p2 * p;
    p3 * (p * (6.0 * p - 15.0) + 10.0)
  }
}

#[test]
fn test_in_out_smoother() {
  // InOut(0) = 0
  assert_eq!(InOutSmoother.y(0.0), 0.0);
  // InOut(0.5) = 0.5 (symmetric)
  assert_eq!(InOutSmoother.y(0.5), 0.5);
  // InOut(1) = 1
  assert_eq!(InOutSmoother.y(1.0), 1.0);
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
/// use eazy::interpolation::polynomial::smootherstep::smootherstep;
///
/// let p = smootherstep(0.25, 0.0, 1.0);
///
/// assert_eq!(p, 0.103515625);
/// ```
///
/// #### notes.
///
/// The formula was suggested by Ken Perlin. For more information about the
/// formula go to the [wiki](<https://en.wikipedia.org/wiki/Smoothstep>)
#[inline(always)]
pub fn smootherstep(p: f32, x0: f32, x1: f32) -> f32 {
  let mut p = (p - x0) / (x1 - x0);

  p = p.clamp(0.0, 1.0);
  p * p * p * (p * (6.0 * p - 15.0) + 10.0)
}
