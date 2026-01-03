//! # The Back Curve.

use crate::easing::Curve;

const C1: f32 = 1.70158;
const C2: f32 = C1 * 1.525;

/// ### The [`InBack`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::standard::back::InBack;
///
/// let p = InBack.y(1.0);
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
  let p = InBack.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutBack`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::standard::back::OutBack;
///
/// let p = OutBack.y(1.0);
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
  let p = OutBack.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutBack`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::standard::back::InOutBack;
///
/// let p = InOutBack.y(1.0);
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
    let k = C1 * C2;

    if t < 1.0 {
      return p * t * (t * (k + 1.0) - k);
    }

    1.0 + 2.0 * m * m * (2.0 * m * (k + 1.0) + k)
  }
}

#[test]
fn test_in_out_back() {
  let p = InOutBack.y(1.0);

  assert_eq!(p, 1.0);
}
