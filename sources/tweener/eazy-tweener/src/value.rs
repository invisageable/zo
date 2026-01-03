//! Tweenable values for animation interpolation.
//!
//! The [`Tweenable`] trait defines types that can be interpolated between
//! two values using linear interpolation modified by easing functions.

/// A value that can be interpolated between two points.
///
/// Types implementing this trait can be animated using [`Tween`].
///
/// # Examples
///
/// ```rust
/// use eazy_tweener::Tweenable;
///
/// let a = 0.0_f32;
/// let b = 100.0_f32;
/// let mid = a.lerp(b, 0.5);
///
/// assert_eq!(mid, 50.0);
/// ```
pub trait Tweenable: Copy + Send + Sync + 'static {
  /// Linearly interpolate between `self` and `other` by factor `t`.
  ///
  /// When `t = 0.0`, returns `self`.
  /// When `t = 1.0`, returns `other`.
  /// Values outside `[0, 1]` extrapolate beyond the endpoints.
  fn lerp(self, other: Self, t: f32) -> Self;
}

// --- Scalar Implementations ---

impl Tweenable for f32 {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    self + (other - self) * t
  }
}

impl Tweenable for f64 {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    self + (other - self) * t as f64
  }
}

impl Tweenable for i32 {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    (self as f32 + (other - self) as f32 * t).round() as i32
  }
}

impl Tweenable for u32 {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    (self as f32 + (other as f32 - self as f32) * t).round() as u32
  }
}

// --- Array Implementations (Vec2, Vec3, Vec4, etc.) ---

impl Tweenable for [f32; 2] {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    [
      self[0] + (other[0] - self[0]) * t,
      self[1] + (other[1] - self[1]) * t,
    ]
  }
}

impl Tweenable for [f32; 3] {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    [
      self[0] + (other[0] - self[0]) * t,
      self[1] + (other[1] - self[1]) * t,
      self[2] + (other[2] - self[2]) * t,
    ]
  }
}

impl Tweenable for [f32; 4] {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    [
      self[0] + (other[0] - self[0]) * t,
      self[1] + (other[1] - self[1]) * t,
      self[2] + (other[2] - self[2]) * t,
      self[3] + (other[3] - self[3]) * t,
    ]
  }
}

impl Tweenable for [f64; 2] {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    let t = t as f64;

    [
      self[0] + (other[0] - self[0]) * t,
      self[1] + (other[1] - self[1]) * t,
    ]
  }
}

impl Tweenable for [f64; 3] {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    let t = t as f64;

    [
      self[0] + (other[0] - self[0]) * t,
      self[1] + (other[1] - self[1]) * t,
      self[2] + (other[2] - self[2]) * t,
    ]
  }
}

impl Tweenable for [f64; 4] {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    let t = t as f64;

    [
      self[0] + (other[0] - self[0]) * t,
      self[1] + (other[1] - self[1]) * t,
      self[2] + (other[2] - self[2]) * t,
      self[3] + (other[3] - self[3]) * t,
    ]
  }
}

// --- Tuple Implementations ---

impl Tweenable for (f32, f32) {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    (
      self.0 + (other.0 - self.0) * t,
      self.1 + (other.1 - self.1) * t,
    )
  }
}

impl Tweenable for (f32, f32, f32) {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    (
      self.0 + (other.0 - self.0) * t,
      self.1 + (other.1 - self.1) * t,
      self.2 + (other.2 - self.2) * t,
    )
  }
}

impl Tweenable for (f32, f32, f32, f32) {
  #[inline(always)]
  fn lerp(self, other: Self, t: f32) -> Self {
    (
      self.0 + (other.0 - self.0) * t,
      self.1 + (other.1 - self.1) * t,
      self.2 + (other.2 - self.2) * t,
      self.3 + (other.3 - self.3) * t,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_f32_lerp() {
    assert_eq!(0.0_f32.lerp(100.0, 0.0), 0.0);
    assert_eq!(0.0_f32.lerp(100.0, 0.5), 50.0);
    assert_eq!(0.0_f32.lerp(100.0, 1.0), 100.0);
  }

  #[test]
  fn test_array_lerp() {
    let a = [0.0_f32, 0.0, 0.0];
    let b = [100.0_f32, 200.0, 300.0];
    let mid = a.lerp(b, 0.5);

    assert_eq!(mid, [50.0, 100.0, 150.0]);
  }

  #[test]
  fn test_extrapolation() {
    // t > 1.0 should extrapolate
    assert_eq!(0.0_f32.lerp(100.0, 1.5), 150.0);
    // t < 0.0 should extrapolate backward
    assert_eq!(0.0_f32.lerp(100.0, -0.5), -50.0);
  }
}
