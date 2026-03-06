//! # The Back Curve.

use crate::easing::Curve;

const C1: f32 = 1.70158;
const C2: f32 = C1 * 1.525;

/// ### The [`InBack`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InBack.y(1.0);
///
/// assert_eq!(p, 1.0);
/// ```
#[derive(Debug)]
pub struct InBack;

impl Curve for InBack {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let k = C1;

    p * p * (p * (k + 1.0) - k)
  }
}

#[test]
fn test_in_back() {
  assert_eq!(InBack.y(0.0), 0.0);
  assert_eq!(InBack.y(1.0), 1.0);

  let p = InBack.y(0.5);

  assert!((p - (-0.08770)).abs() < 1e-4, "InBack(0.5) = {p}");
}

/// ### The [`OutBack`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::OutBack.y(1.0);
///
/// assert_eq!(p, 1.0);
/// ```
#[derive(Debug)]
pub struct OutBack;

impl Curve for OutBack {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let k = C1;

    1.0 + m * m * (m * (k + 1.0) + k)
  }
}

#[test]
fn test_out_back() {
  assert_eq!(OutBack.y(0.0), 0.0);
  assert_eq!(OutBack.y(1.0), 1.0);

  let p = OutBack.y(0.5);

  assert!((p - 1.08770).abs() < 1e-4, "OutBack(0.5) = {p}");
}

/// ### The [`InOutBack`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::InOutBack.y(1.0);
///
/// assert_eq!(p, 1.0);
/// ```
#[derive(Debug)]
pub struct InOutBack;

impl Curve for InOutBack {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let m = p - 1.0;
    let t = p * 2.0;
    let k = C2;

    if t < 1.0 {
      return p * t * (t * (k + 1.0) - k);
    }

    1.0 + 2.0 * m * m * (2.0 * m * (k + 1.0) + k)
  }
}

#[test]
fn test_in_out_back() {
  assert_eq!(InOutBack.y(0.0), 0.0);
  assert_eq!(InOutBack.y(1.0), 1.0);
  assert_eq!(InOutBack.y(0.5), 0.5);

  let p = InOutBack.y(0.25);

  assert!((p - (-0.09968)).abs() < 1e-4, "InOutBack(0.25) = {p}");
}
