//! # The Bounce Curve.

use crate::easing::Curve;

/// ### The [InBounce] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::standard::bounce::InBounce;
///
/// let p = InBounce.y(1.0);
/// ```
#[derive(Debug)]
pub struct InBounce;

impl Curve for InBounce {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    1.0 - OutBounce.y(1.0 - p)
  }
}

#[test]
fn test_in_bounce() {
  let p = InBounce.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### [`OutBounce`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::standard::bounce::OutBounce;
/// let p = OutBounce.y(1.0);
/// ```
#[derive(Debug)]
pub struct OutBounce;

impl Curve for OutBounce {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let t;
    let r = 1.0 / 2.75; // -- reciprocal.
    let k1 = r; // ---------- 36.36%
    let k2 = 2.0 * r; // ---- 72.72%
    let k3 = 1.5 * r; // ---- 54.54%
    let k4 = 2.5 * r; // ---- 90.90%
    let k5 = 2.25 * r; // --- 81.81%
    let k6 = 2.625 * r; // -- 95.45%
    let k0 = 7.5625;

    if p < k1 {
      k0 * p * p
    } else if p < k2 {
      t = p - k3;

      k0 * t * t + 0.75 // ------- 48/64
    } else if p < k4 {
      t = p - k5;

      k0 * t * t + 0.9375 // ----- 60/64
    } else {
      t = p - k6;

      k0 * t * t + 0.984375 // --- 63/64
    }
  }
}

#[test]
fn test_out_bounce() {
  let p = OutBounce.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutBounce`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::standard::bounce::InOutBounce;
///
/// let p = InOutBounce.y(1.0);
/// ```
#[derive(Debug)]
pub struct InOutBounce;

impl Curve for InOutBounce {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let t = p * 2.0;

    if t < 1.0 {
      return 0.5 - 0.5 * OutBounce.y(1.0 - t);
    }

    0.5 + 0.5 * OutBounce.y(t - 1.0)
  }
}

#[test]
fn test_in_out_bounce() {
  let p = InOutBounce.y(1.0);

  assert_eq!(p, 1.0);
}
