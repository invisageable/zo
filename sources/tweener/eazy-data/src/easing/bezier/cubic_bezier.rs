//! # The Cubic Bézier Curve.
//!
//! CSS-compliant cubic bezier easing using Newton-Raphson iteration.
//! More information [here](https://drafts.csswg.org/css-easing/#cubic-bezier-easing-functions).

use crate::easing::Curve;

/// Sample table size for initial guess optimization.
const SAMPLE_TABLE_SIZE: usize = 11;

/// Newton-Raphson iteration count.
const NEWTON_ITERATIONS: usize = 4;

/// Minimum slope for Newton-Raphson (below this, use subdivision).
const NEWTON_MIN_SLOPE: f32 = 0.001;

/// Binary subdivision precision.
const SUBDIVISION_PRECISION: f32 = 0.0000001;

/// Maximum binary subdivision iterations.
const SUBDIVISION_MAX_ITERATIONS: usize = 10;

/// The [`CubicBezier`] Easing Function.
///
/// Implements CSS `cubic-bezier()` timing function.
/// Given an input x (progress), solves for t where bezier_x(t) = x,
/// then returns bezier_y(t).
///
/// #### notes.
///
/// See also [`crate::easing::Easing`].
#[derive(Clone, Copy, Debug)]
pub struct CubicBezier {
  /// Control point 1 x.
  p1x: f32,
  /// Control point 1 y.
  p1y: f32,
  /// Control point 2 x.
  p2x: f32,
  /// Control point 2 y.
  p2y: f32,
  /// Precomputed sample table for x values.
  samples: [f32; SAMPLE_TABLE_SIZE],
}

impl CubicBezier {
  // Bezier coefficient A.
  #[inline(always)]
  fn a(p1: f32, p2: f32) -> f32 {
    1.0 - 3.0 * p2 + 3.0 * p1
  }

  // Bezier coefficient B.
  #[inline(always)]
  fn b(p1: f32, p2: f32) -> f32 {
    3.0 * p2 - 6.0 * p1
  }

  // Bezier coefficient C.
  #[inline(always)]
  fn c(p1: f32) -> f32 {
    3.0 * p1
  }

  // Evaluate bezier at parameter t.
  #[inline(always)]
  fn bezier_at(t: f32, p1: f32, p2: f32) -> f32 {
    ((Self::a(p1, p2) * t + Self::b(p1, p2)) * t + Self::c(p1)) * t
  }

  // Evaluate bezier derivative at parameter t.
  #[inline(always)]
  fn bezier_slope(t: f32, p1: f32, p2: f32) -> f32 {
    3.0 * Self::a(p1, p2) * t * t + 2.0 * Self::b(p1, p2) * t + Self::c(p1)
  }

  // Newton-Raphson iteration to find t for given x.
  #[inline(always)]
  fn newton_raphson(&self, x: f32, guess: f32) -> f32 {
    let mut t = guess;

    for _ in 0..NEWTON_ITERATIONS {
      let slope = Self::bezier_slope(t, self.p1x, self.p2x);

      if slope == 0.0 {
        return t;
      }

      let current_x = Self::bezier_at(t, self.p1x, self.p2x) - x;

      t -= current_x / slope;
    }

    t
  }

  // Binary subdivision to find t for given x.
  #[inline(always)]
  fn binary_subdivide(&self, x: f32, mut a: f32, mut b: f32) -> f32 {
    let mut t = 0.0;

    for _ in 0..SUBDIVISION_MAX_ITERATIONS {
      t = a + (b - a) / 2.0;

      let current_x = Self::bezier_at(t, self.p1x, self.p2x) - x;

      if current_x.abs() < SUBDIVISION_PRECISION {
        return t;
      }

      if current_x > 0.0 {
        b = t;
      } else {
        a = t;
      }
    }

    t
  }

  // Find parameter t for given x using sample table + refinement.
  #[inline(always)]
  fn t_for_x(&self, x: f32) -> f32 {
    let sample_step = 1.0 / (SAMPLE_TABLE_SIZE - 1) as f32;

    // Find interval in sample table.
    let mut interval_start = 0.0;
    let mut current_sample = 1;

    while current_sample < SAMPLE_TABLE_SIZE - 1
      && self.samples[current_sample] <= x
    {
      interval_start += sample_step;
      current_sample += 1;
    }

    current_sample -= 1;

    // Linear interpolation for initial guess.
    let dist = (x - self.samples[current_sample])
      / (self.samples[current_sample + 1] - self.samples[current_sample]);
    let guess = interval_start + dist * sample_step;

    // Refine using Newton-Raphson or binary subdivision.
    let initial_slope = Self::bezier_slope(guess, self.p1x, self.p2x);

    if initial_slope >= NEWTON_MIN_SLOPE {
      self.newton_raphson(x, guess)
    } else if initial_slope == 0.0 {
      guess
    } else {
      self.binary_subdivide(x, interval_start, interval_start + sample_step)
    }
  }

  /// Creates a [`CubicBezier`] curve from two control points.
  ///
  /// `p0` and `p3` are fixed to (0,0) and (1,1). Control points `p1` and `p2`
  /// define the curve shape.
  ///
  /// #### params.
  ///
  /// |       |                                              |
  /// |:------|----------------------------------------------|
  /// | `p1x` | The position of the `x-axis` of `p1` control |
  /// | `p1y` | The position of the `y-axis` of `p1` control |
  /// | `p2x` | The position of the `x-axis` of `p2` control |
  /// | `p2y` | The position of the `y-axis` of `p2` control |
  ///
  /// #### examples.
  ///
  /// ```rust
  /// use eazy::Curve;
  /// use eazy::bezier::cubic::CubicBezier;
  ///
  /// // CSS ease timing function
  /// let ease = CubicBezier::curve(0.25, 0.1, 0.25, 1.0);
  ///
  /// assert_eq!(ease.y(0.0), 0.0);
  /// assert_eq!(ease.y(1.0), 1.0);
  /// // At x=0.5, CSS ease returns ~0.8 (steep middle)
  /// ```
  #[inline(always)]
  pub fn curve(p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> Self {
    // Precompute sample table for x values.
    let mut samples = [0.0; SAMPLE_TABLE_SIZE];

    for (i, sample) in samples.iter_mut().enumerate() {
      let t = i as f32 / (SAMPLE_TABLE_SIZE - 1) as f32;

      *sample = Self::bezier_at(t, p1x, p2x);
    }

    Self {
      p1x,
      p1y,
      p2x,
      p2y,
      samples,
    }
  }

  /// Creates a [`CubicBezier::curve`] based on CSS `ease`.
  /// Equivalent to `cubic-bezier(0.25, 0.1, 0.25, 1.0)`.
  #[inline(always)]
  pub fn ease() -> Self {
    Self::curve(0.25, 0.1, 0.25, 1.0)
  }

  /// Creates a [`CubicBezier::curve`] based on CSS `ease-in`.
  /// Equivalent to `cubic-bezier(0.42, 0, 1, 1)`.
  #[inline(always)]
  pub fn in_ease() -> Self {
    Self::curve(0.42, 0.0, 1.0, 1.0)
  }

  /// Creates a [`CubicBezier::curve`] based on CSS `ease-out`.
  /// Equivalent to `cubic-bezier(0, 0, 0.58, 1)`.
  #[inline(always)]
  pub fn out_ease() -> Self {
    Self::curve(0.0, 0.0, 0.58, 1.0)
  }

  /// Creates a [`CubicBezier::curve`] based on CSS `ease-in-out`.
  /// Equivalent to `cubic-bezier(0.42, 0, 0.58, 1)`.
  #[inline(always)]
  pub fn in_out_ease() -> Self {
    Self::curve(0.42, 0.0, 0.58, 1.0)
  }
}

impl Curve for CubicBezier {
  #[inline(always)]
  fn y(&self, x: f32) -> f32 {
    // Handle edge cases.
    if x <= 0.0 {
      return 0.0;
    }

    if x >= 1.0 {
      return 1.0;
    }

    // Linear case: p1x == p1y && p2x == p2y.
    if self.p1x == self.p1y && self.p2x == self.p2y {
      return x;
    }

    // Find t for given x, then evaluate y at that t.
    let t = self.t_for_x(x);

    Self::bezier_at(t, self.p1y, self.p2y)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_cubic_bezier_endpoints() {
    let ease = CubicBezier::ease();

    assert_eq!(ease.y(0.0), 0.0);
    assert_eq!(ease.y(1.0), 1.0);
  }

  #[test]
  fn test_cubic_bezier_ease_midpoint() {
    // CSS ease at x=0.5 should be approximately 0.8
    // (the curve accelerates quickly then decelerates).
    let ease = CubicBezier::ease();
    let y = ease.y(0.5);

    assert!(y > 0.75 && y < 0.85, "ease(0.5) = {y}, expected ~0.8");
  }

  #[test]
  fn test_cubic_bezier_linear() {
    // Linear: cubic-bezier(0, 0, 1, 1).
    let linear = CubicBezier::curve(0.0, 0.0, 1.0, 1.0);

    assert!((linear.y(0.25) - 0.25).abs() < 0.01);
    assert!((linear.y(0.5) - 0.5).abs() < 0.01);
    assert!((linear.y(0.75) - 0.75).abs() < 0.01);
  }

  #[test]
  fn test_cubic_bezier_ease_in() {
    // ease-in starts slow, ends fast.
    let ease_in = CubicBezier::in_ease();

    // At x=0.5, y should be less than 0.5.
    assert!(ease_in.y(0.5) < 0.5);
  }

  #[test]
  fn test_cubic_bezier_ease_out() {
    // ease-out starts fast, ends slow.
    let ease_out = CubicBezier::out_ease();

    // At x=0.5, y should be greater than 0.5.
    assert!(ease_out.y(0.5) > 0.5);
  }

  #[test]
  fn test_cubic_bezier_ease_in_out() {
    // ease-in-out is symmetric around 0.5.
    let ease_in_out = CubicBezier::in_out_ease();
    let y = ease_in_out.y(0.5);

    // Should be close to 0.5 at midpoint.
    assert!((y - 0.5).abs() < 0.05, "ease-in-out(0.5) = {y}");
  }
}
