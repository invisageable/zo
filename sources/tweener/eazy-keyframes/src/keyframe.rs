//! Keyframe definition for animation tracks.
//!
//! A [`Keyframe`] represents a value at a specific point in time,
//! with optional easing to control interpolation to this keyframe.

use eazy_data::{Curve, Easing};
use eazy_tweener::Tweenable;

/// A keyframe representing a value at a specific time.
///
/// # Type Parameters
///
/// - `T`: The value type, must implement [`Tweenable`].
///
/// # Examples
///
/// ```rust
/// use eazy_keyframes::Keyframe;
/// use eazy_data::Easing;
///
/// // Keyframe at 50% with OutBounce easing
/// let kf = Keyframe::new(0.5, 100.0_f32)
///   .with_easing(Easing::OutBounce);
/// ```
#[derive(Debug, Clone)]
pub struct Keyframe<T: Tweenable> {
  /// Time position (normalized 0.0-1.0 or absolute seconds).
  time: f32,
  /// Value at this keyframe.
  value: T,
  /// Easing function used to interpolate TO this keyframe.
  /// If None, uses linear interpolation.
  easing: Option<Easing>,
}

impl<T: Tweenable> Keyframe<T> {
  /// Create a new keyframe at the given time with the given value.
  pub fn new(time: f32, value: T) -> Self {
    Self {
      time,
      value,
      easing: None,
    }
  }

  /// Create a keyframe with easing.
  pub fn with_easing(mut self, easing: Easing) -> Self {
    self.easing = Some(easing);
    self
  }

  /// Set the easing function.
  pub fn set_easing(&mut self, easing: Option<Easing>) {
    self.easing = easing;
  }

  /// Get the time position.
  pub fn time(&self) -> f32 {
    self.time
  }

  /// Get the value.
  pub fn value(&self) -> T {
    self.value
  }

  /// Get the easing function.
  pub fn easing(&self) -> Option<&Easing> {
    self.easing.as_ref()
  }

  /// Check if this keyframe has custom easing.
  pub fn has_easing(&self) -> bool {
    self.easing.is_some()
  }

  /// Interpolate from this keyframe to the next at the given time.
  ///
  /// Uses the easing function from `next` (since easing controls
  /// interpolation TO a keyframe).
  ///
  /// # Behavior
  ///
  /// - If `time <= self.time`: returns `self.value`
  /// - If `time >= next.time`: returns `next.value`
  /// - If `next.time <= self.time`: returns `next.value` (invalid order)
  /// - Otherwise: interpolates using `next.easing` (or linear if None)
  ///
  /// # Examples
  ///
  /// ```rust
  /// use eazy_keyframes::Keyframe;
  /// use eazy_data::Easing;
  ///
  /// let kf1 = Keyframe::new(0.0, 0.0_f32);
  /// let kf2 = Keyframe::new(1.0, 100.0_f32).with_easing(Easing::OutBounce);
  ///
  /// let value = kf1.tween_to(&kf2, 0.5);
  /// ```
  #[inline]
  pub fn tween_to(&self, next: &Keyframe<T>, time: f32) -> T {
    // Before this keyframe.
    if time <= self.time {
      return self.value;
    }

    // After next keyframe.
    if time >= next.time {
      return next.value;
    }

    // Invalid ordering (next is before self).
    if next.time <= self.time {
      return next.value;
    }

    // Normalize time to [0, 1] between the two keyframes.
    let t = (time - self.time) / (next.time - self.time);

    // Apply easing from the target keyframe.
    let eased_t = match &next.easing {
      Some(easing) => easing.y(t),
      None => t, // Linear.
    };

    self.value.lerp(next.value, eased_t)
  }
}

impl<T: Tweenable> PartialEq for Keyframe<T> {
  fn eq(&self, other: &Self) -> bool {
    self.time == other.time
  }
}

// --- From implementations for tuple syntax ---

/// Create keyframe from `(time, value)` tuple with linear easing.
impl<T: Tweenable> From<(f32, T)> for Keyframe<T> {
  #[inline]
  fn from((time, value): (f32, T)) -> Self {
    Keyframe::new(time, value)
  }
}

/// Create keyframe from `(time, value, easing)` tuple.
impl<T: Tweenable> From<(f32, T, Easing)> for Keyframe<T> {
  #[inline]
  fn from((time, value, easing): (f32, T, Easing)) -> Self {
    Keyframe::new(time, value).with_easing(easing)
  }
}

impl<T: Tweenable> PartialOrd for Keyframe<T> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.time.partial_cmp(&other.time)
  }
}

/// Convenience function to create a keyframe.
pub fn keyframe<T: Tweenable>(time: f32, value: T) -> Keyframe<T> {
  Keyframe::new(time, value)
}

/// Convenience function to create a keyframe with easing.
pub fn keyframe_eased<T: Tweenable>(
  time: f32,
  value: T,
  easing: Easing,
) -> Keyframe<T> {
  Keyframe::new(time, value).with_easing(easing)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_keyframe_creation() {
    let kf = Keyframe::new(0.5, 100.0_f32);

    assert_eq!(kf.time(), 0.5);
    assert_eq!(kf.value(), 100.0);
    assert!(kf.easing().is_none());
  }

  #[test]
  fn test_keyframe_with_easing() {
    let kf = Keyframe::new(0.5, 100.0_f32).with_easing(Easing::OutBounce);

    assert!(kf.has_easing());
  }

  #[test]
  fn test_keyframe_ordering() {
    let kf1 = Keyframe::new(0.2, 50.0_f32);
    let kf2 = Keyframe::new(0.5, 100.0_f32);
    let kf3 = Keyframe::new(0.8, 150.0_f32);

    assert!(kf1 < kf2);
    assert!(kf2 < kf3);
    assert!(kf1 < kf3);
  }

  #[test]
  fn test_keyframe_array() {
    let kf = Keyframe::new(0.5, [1.0_f32, 2.0, 3.0]);

    assert_eq!(kf.value(), [1.0, 2.0, 3.0]);
  }

  #[test]
  fn test_tween_to_linear() {
    let kf1 = Keyframe::new(0.0, 0.0_f32);
    let kf2 = Keyframe::new(1.0, 100.0_f32);

    // Linear interpolation (no easing on kf2).
    assert_eq!(kf1.tween_to(&kf2, 0.0), 0.0);
    assert_eq!(kf1.tween_to(&kf2, 0.5), 50.0);
    assert_eq!(kf1.tween_to(&kf2, 1.0), 100.0);
  }

  #[test]
  fn test_tween_to_clamping() {
    let kf1 = Keyframe::new(0.2, 10.0_f32);
    let kf2 = Keyframe::new(0.8, 80.0_f32);

    // Before kf1 -> returns kf1.value.
    assert_eq!(kf1.tween_to(&kf2, 0.0), 10.0);
    assert_eq!(kf1.tween_to(&kf2, 0.1), 10.0);

    // After kf2 -> returns kf2.value.
    assert_eq!(kf1.tween_to(&kf2, 0.9), 80.0);
    assert_eq!(kf1.tween_to(&kf2, 1.0), 80.0);
  }

  #[test]
  fn test_tween_to_with_easing() {
    let kf1 = Keyframe::new(0.0, 0.0_f32);
    let kf2 = Keyframe::new(1.0, 100.0_f32).with_easing(Easing::InQuadratic);

    // InQuadratic at t=0.5 gives 0.25, so value = 25.0.
    let value = kf1.tween_to(&kf2, 0.5);

    assert_eq!(value, 25.0);
  }

  #[test]
  fn test_tween_to_array() {
    let kf1 = Keyframe::new(0.0, [0.0_f32, 0.0, 0.0]);
    let kf2 = Keyframe::new(1.0, [100.0, 200.0, 300.0]);

    let value = kf1.tween_to(&kf2, 0.5);

    assert_eq!(value, [50.0, 100.0, 150.0]);
  }
}
