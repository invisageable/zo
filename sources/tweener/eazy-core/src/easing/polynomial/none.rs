//! # The Constant Curve.
//!
//! An algebraic curve of degree zero.
//!
//! #### formula.
//!
//! `p^0`

use crate::easing::Curve;

/// ### The [`None`] Easing Function.
///
/// It's used explicitly as a placeholder to informs that an animation is not
/// currently active. In our case, we define the none constant value to `1`
/// instead of `minus infinity`.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::power::none::None;
///
/// let p = None.y(0.0);
/// ```
#[derive(Debug)]
pub struct None;

impl Curve for None {
  #[inline(always)]
  fn y(&self, _p: f32) -> f32 {
    1.0
  }
}

#[test]
fn test_none() {
  assert!(None.y(0.0) == 1.0);
  assert!(None.y(0.5) == 1.0);
  assert!(None.y(1.0) == 1.0);
}
