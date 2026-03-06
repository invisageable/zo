//! # The Linear Curve.
//!
//! An algebraic curve of degree one.
//!
//! #### formula.
//!
//! `p`

use crate::easing::Curve;

/// ### The [`Linear`] Easing Function.
///
/// #### examples.
///
/// ```
/// use eazy_core::{Curve, Easing};
///
/// let p = Easing::Linear.y(1.0);
/// ```
#[derive(Debug)]
pub struct Linear;

impl Curve for Linear {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    p
  }
}

#[test]
fn test_linear() {
  let p = Linear.y(100.0);

  assert_eq!(p, 100.0);
}
