//! Quartic Smoothstep (C1 continuity).
//!
//! From Inigo Quilez: https://iquilezles.org/articles/smoothsteps/
//!
//! Uses only even powers of x, which is useful when x represents
//! distance (avoiding square roots in distance calculations).
//!
//! - **Quartic**: `x²(2-x²)` - the smoothstep function
//! - **InvQuartic**: `sqrt(1-sqrt(1-x))` - the inverse function

use crate::easing::Curve;

use libm::sqrtf;

/// Quartic Smoothstep: `x²(2-x²)`
///
/// C1 continuous smoothstep using only even powers.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::quartic::Quartic;
///
/// let p = Quartic.y(0.5);
/// assert!((p - 0.4375).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct Quartic;

impl Curve for Quartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p * p * (2.0 - p * p)
  }
}

#[test]
fn test_quartic() {
  // Quartic(0) = 0
  assert_eq!(Quartic.y(0.0), 0.0);
  // Quartic(1) = 1
  assert_eq!(Quartic.y(1.0), 1.0);
  // Quartic(0.5) = 0.25 * 1.75 = 0.4375
  assert!((Quartic.y(0.5) - 0.4375).abs() < 0.0001);
}

/// Inverse Quartic Smoothstep: `sqrt(1-sqrt(1-x))`
///
/// Maps output values back to input values.
/// `InvQuartic(Quartic(x)) = x`
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::interpolation::polynomial::quartic::{Quartic, InvQuartic};
///
/// let x = 0.3;
/// let y = Quartic.y(x);
/// let x_back = InvQuartic.y(y);
/// assert!((x - x_back).abs() < 0.0001);
/// ```
#[derive(Debug)]
pub struct InvQuartic;

impl Curve for InvQuartic {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    sqrtf(1.0 - sqrtf(1.0 - p))
  }
}

#[test]
fn test_inv_quartic() {
  // InvQuartic(0) = 0
  assert_eq!(InvQuartic.y(0.0), 0.0);
  // InvQuartic(1) = 1
  assert_eq!(InvQuartic.y(1.0), 1.0);
  // Round-trip: InvQuartic(Quartic(0.3)) ≈ 0.3
  let x = 0.3;
  let round_trip = InvQuartic.y(Quartic.y(x));
  assert!((x - round_trip).abs() < 0.0001);
}
