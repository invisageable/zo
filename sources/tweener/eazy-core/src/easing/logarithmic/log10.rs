//! # The Log10 Curve.

use crate::easing::Curve;

use libm::log10f;

/// ### The [`InLog10`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::logarithmic::log10::InLog10;
///
/// let p = InLog10.y(0.25);
/// ```
#[derive(Debug)]
pub struct InLog10;

impl Curve for InLog10 {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    1.0 - OutLog10.y(1.0 - p)
  }
}

#[test]
fn test_in_log10() {
  let p = InLog10.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`OutLog10`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::logarithmic::log10::OutLog10;
///
/// let p = OutLog10.y(0.25);
/// ```
#[derive(Debug)]
pub struct OutLog10;

impl Curve for OutLog10 {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    log10f((p * 9.0) + 1.0)
  }
}

#[test]
fn test_out_log10() {
  let p = OutLog10.y(1.0);

  assert_eq!(p, 1.0);
}

/// ### The [`InOutLog10`] Easing Function.
///
/// #### examples.
///
/// ```rust
/// use eazy::Curve;
/// use eazy::logarithmic::log10::InOutLog10;
///
/// let p = InOutLog10.y(0.25);
/// ```
#[derive(Debug)]
pub struct InOutLog10;

impl Curve for InOutLog10 {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let t = p * 2.0;

    if t < 1.0 {
      return 0.5 - 0.5 * OutLog10.y(1.0 - t);
    }

    0.5 + 0.5 * OutLog10.y(t - 1.0)
  }
}

#[test]
fn test_in_out_log10() {
  let p = InOutLog10.y(1.0);

  assert_eq!(p, 1.0);
}
