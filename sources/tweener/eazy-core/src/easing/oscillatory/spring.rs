//! # The Spring Curve.
//!
//! Spring-like easing with exponential decay and oscillation.
//! Creates a bouncy, overshoot effect that settles at the target.
//!
//! #### formula.
//!
//! `1 + e^(-decay * t) * cos(frequency * t)`
//!
//! Default parameters: decay=6.0, frequency=10.0

use crate::easing::Curve;

use libm::{cosf, expf};

/// ### The [`Spring`] Easing Function.
///
/// Spring-like easing that overshoots then settles.
/// Uses exponential decay with cosine oscillation.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::oscillatory::spring::Spring;
///
/// let p = Spring.y(1.0);
/// assert!((p - 1.0).abs() < 0.01);
/// ```
#[derive(Debug)]
pub struct Spring;

impl Curve for Spring {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    // Spring formula: 1 + e^(-decay*t) * cos(frequency*t)
    // decay=6.0, frequency=10.0 gives nice springy feel
    1.0 + expf(-6.0 * p) * cosf(10.0 * p)
  }
}

#[test]
fn test_spring() {
  // Spring(0) = 1 + e^0 * cos(0) = 1 + 1*1 = 2 (overshoots initially)
  assert!((Spring.y(0.0) - 2.0).abs() < 0.0001);
  // Spring(1) ≈ 1 (settles at target)
  assert!((Spring.y(1.0) - 1.0).abs() < 0.01);
}

/// ### The [`SpringConfigurable`] Easing Function.
///
/// Spring easing with configurable decay and frequency parameters.
///
/// #### examples.
///
/// ```
/// use eazy::Curve;
/// use eazy::oscillatory::spring::SpringConfigurable;
///
/// // Stiffer spring with faster decay
/// let spring = SpringConfigurable::new(8.0, 12.0);
/// let p = spring.y(0.5);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SpringConfigurable {
  decay: f32,
  frequency: f32,
}

impl SpringConfigurable {
  /// Create a new configurable spring.
  ///
  /// #### params.
  ///
  /// |             |                                           |
  /// |:------------|:------------------------------------------|
  /// | `decay`     | How quickly oscillations fade (higher=faster) |
  /// | `frequency` | How fast it oscillates (higher=more bounces)  |
  #[inline(always)]
  pub const fn new(decay: f32, frequency: f32) -> Self {
    Self { decay, frequency }
  }
}

impl Curve for SpringConfigurable {
  #[inline(always)]
  fn y(&self, p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    1.0 + expf(-self.decay * p) * cosf(self.frequency * p)
  }
}

#[test]
fn test_spring_configurable() {
  let spring = SpringConfigurable::new(6.0, 10.0);
  // Should behave like default Spring
  assert!((spring.y(0.0) - 2.0).abs() < 0.0001);
  assert!((spring.y(1.0) - 1.0).abs() < 0.01);
}
