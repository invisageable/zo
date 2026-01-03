//! Repeat and yoyo configuration for animations.
//!
//! Controls how animations loop and alternate direction.

/// How many times an animation should repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
  /// Play once, no repeating.
  #[default]
  None,
  /// Repeat a specific number of times after the initial play.
  /// `Count(1)` means play twice total (initial + 1 repeat).
  Count(u32),
  /// Repeat forever.
  Infinite,
}

impl Repeat {
  /// Check if repeating is enabled.
  pub fn is_repeating(&self) -> bool {
    !matches!(self, Self::None)
  }

  /// Check if infinite repeating.
  pub fn is_infinite(&self) -> bool {
    matches!(self, Self::Infinite)
  }

  /// Get the count if finite, None if infinite or none.
  pub fn count(&self) -> Option<u32> {
    match self {
      Self::None => Some(0),
      Self::Count(n) => Some(*n),
      Self::Infinite => None,
    }
  }

  /// Check if more repeats are available.
  pub fn can_repeat(&self, current_iteration: u32) -> bool {
    match self {
      Self::None => false,
      Self::Count(n) => current_iteration < *n,
      Self::Infinite => true,
    }
  }
}

impl From<u32> for Repeat {
  fn from(count: u32) -> Self {
    if count == 0 {
      Self::None
    } else {
      Self::Count(count)
    }
  }
}

impl From<i32> for Repeat {
  fn from(count: i32) -> Self {
    if count < 0 {
      Self::Infinite
    } else if count == 0 {
      Self::None
    } else {
      Self::Count(count as u32)
    }
  }
}

/// Complete repeat configuration for a tween.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatConfig {
  /// How many times to repeat.
  pub repeat: Repeat,
  /// Alternate direction on each repeat (ping-pong effect).
  pub yoyo: bool,
  /// Delay in seconds before each repeat.
  pub delay: f32,
}

impl RepeatConfig {
  /// Create a new repeat config with no repeating.
  pub fn none() -> Self {
    Self::default()
  }

  /// Create a repeat config that repeats `n` times.
  pub fn count(n: u32) -> Self {
    Self {
      repeat: Repeat::Count(n),
      yoyo: false,
      delay: 0.0,
    }
  }

  /// Create a repeat config that repeats infinitely.
  pub fn infinite() -> Self {
    Self {
      repeat: Repeat::Infinite,
      yoyo: false,
      delay: 0.0,
    }
  }

  /// Enable yoyo mode (alternate direction on each repeat).
  pub fn with_yoyo(mut self, yoyo: bool) -> Self {
    self.yoyo = yoyo;
    self
  }

  /// Set delay before each repeat.
  pub fn with_delay(mut self, delay: f32) -> Self {
    self.delay = delay;
    self
  }

  /// Check if this config has any repeating.
  pub fn is_repeating(&self) -> bool {
    self.repeat.is_repeating()
  }

  /// Check if the animation should reverse on the given iteration.
  ///
  /// With yoyo enabled, odd iterations play in reverse.
  pub fn should_reverse(&self, iteration: u32) -> bool {
    self.yoyo && iteration % 2 == 1
  }

  /// Calculate total duration including all repeats.
  ///
  /// Returns `None` for infinite repeats.
  pub fn total_duration(&self, single_duration: f32) -> Option<f32> {
    match self.repeat {
      Repeat::None => Some(single_duration),
      Repeat::Count(n) => {
        let repeat_duration = single_duration + self.delay;

        Some(single_duration + repeat_duration * n as f32)
      }
      Repeat::Infinite => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_repeat_can_repeat() {
    assert!(!Repeat::None.can_repeat(0));
    assert!(Repeat::Count(3).can_repeat(0));
    assert!(Repeat::Count(3).can_repeat(2));
    assert!(!Repeat::Count(3).can_repeat(3));
    assert!(Repeat::Infinite.can_repeat(1000));
  }

  #[test]
  fn test_repeat_from_i32() {
    assert_eq!(Repeat::from(-1), Repeat::Infinite);
    assert_eq!(Repeat::from(0), Repeat::None);
    assert_eq!(Repeat::from(5), Repeat::Count(5));
  }

  #[test]
  fn test_yoyo_direction() {
    let config = RepeatConfig::count(3).with_yoyo(true);

    assert!(!config.should_reverse(0)); // forward
    assert!(config.should_reverse(1)); // reverse
    assert!(!config.should_reverse(2)); // forward
    assert!(config.should_reverse(3)); // reverse
  }

  #[test]
  fn test_total_duration() {
    let config = RepeatConfig::count(2).with_delay(0.5);

    // 1.0 + (1.0 + 0.5) * 2 = 1.0 + 3.0 = 4.0
    assert_eq!(config.total_duration(1.0), Some(4.0));
  }

  #[test]
  fn test_infinite_duration() {
    let config = RepeatConfig::infinite();

    assert_eq!(config.total_duration(1.0), None);
  }
}
