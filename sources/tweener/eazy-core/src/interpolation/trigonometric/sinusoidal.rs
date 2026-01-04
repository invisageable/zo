//! Trigonometric Smoothstep (C∞ continuity).
//!
//! From Inigo Quilez: https://iquilezles.org/articles/smoothsteps/
//!
//! Uses trigonometric functions for infinitely smooth transitions.
//! Has C∞ continuity (all derivatives are zero at endpoints).
//!
//! - **Sinusoidal**: `0.5 - 0.5*cos(π*x)` - the smoothstep function
//! - **InvSinusoidal**: `acos(1-2*x)/π` - the inverse function

use crate::easing::Curve;

use libm::{acosf, cosf};

/// Trigonometric Smoothstep: `0.5 - 0.5*cos(π*x)`
///
/// C∞ continuous smoothstep using cosine.
/// All derivatives are zero at x=0 and x=1.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::trigonometric::sinusoidal::Sinusoidal;
///
/// let p = Sinusoidal.y(0.5);
/// assert!((p - 0.5).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct Sinusoidal;

impl Curve for Sinusoidal {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    0.5 - 0.5 * cosf(core::f32::consts::PI * p)
  }
}

#[test]
fn test_sinusoidal() {
  // Sinusoidal(0) = 0
  assert_eq!(Sinusoidal.y(0.0), 0.0);
  // Sinusoidal(1) = 1
  assert!((Sinusoidal.y(1.0) - 1.0).abs() < 0.0001);
  // Sinusoidal(0.5) = 0.5 (symmetric)
  assert!((Sinusoidal.y(0.5) - 0.5).abs() < 0.0001);
}

/// Inverse Trigonometric Smoothstep: `acos(1-2*x)/π`
///
/// Maps output values back to input values.
/// `InvSinusoidal(Sinusoidal(x)) = x`
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::trigonometric::sinusoidal::{Sinusoidal, InvSinusoidal};
///
/// let x = 0.3;
/// let y = Sinusoidal.y(x);
/// let x_back = InvSinusoidal.y(y);
/// assert!((x - x_back).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct InvSinusoidal;

impl Curve for InvSinusoidal {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    acosf(1.0 - 2.0 * p) / core::f32::consts::PI
  }
}

#[test]
fn test_inv_sinusoidal() {
  // InvSinusoidal(0) = 0
  assert_eq!(InvSinusoidal.y(0.0), 0.0);
  // InvSinusoidal(1) = 1
  assert!((InvSinusoidal.y(1.0) - 1.0).abs() < 0.0001);
  // Round-trip: InvSinusoidal(Sinusoidal(0.3)) ≈ 0.3
  let x = 0.3;
  let round_trip = InvSinusoidal.y(Sinusoidal.y(x));
  assert!((x - round_trip).abs() < 0.0001);
}
