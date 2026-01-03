//! Stagger system for cascading animations.
//!
//! Staggers offset the start time of multiple animations to create
//! cascading effects like domino falls or wave animations.

use eazy_data::Curve;
use eazy_data::Easing;

/// Direction from which staggering originates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StaggerFrom {
  /// Stagger from first to last (index 0 starts first).
  #[default]
  Start,
  /// Stagger from last to first (last index starts first).
  End,
  /// Stagger from center outward (middle starts first).
  Center,
  /// Stagger from edges inward (first and last start first).
  Edges,
  /// Random stagger order.
  Random,
}

impl StaggerFrom {
  /// Calculate the stagger index for a given position.
  ///
  /// Returns a value from 0.0 to 1.0 representing the relative
  /// position in the stagger sequence.
  pub fn index_factor(&self, index: usize, total: usize) -> f32 {
    if total <= 1 {
      return 0.0;
    }

    let i = index as f32;
    let n = (total - 1) as f32;

    match self {
      Self::Start => i / n,
      Self::End => (n - i) / n,
      Self::Center => {
        let center = n / 2.0;
        let distance = (i - center).abs();

        distance / center
      }
      Self::Edges => {
        let center = n / 2.0;
        let distance = (i - center).abs();

        1.0 - (distance / center)
      }
      Self::Random => {
        // For deterministic "random", use a simple hash
        let hash = ((index as u32).wrapping_mul(2654435761)) as f32;

        hash / u32::MAX as f32
      }
    }
  }
}

/// Configuration for staggering multiple animations.
///
/// # Examples
///
/// ```rust
/// use eazy_tweener::{Stagger, StaggerFrom};
///
/// // Each animation starts 0.1s after the previous
/// let stagger = Stagger::each(0.1);
///
/// // Stagger from center outward
/// let stagger = Stagger::each(0.1).from(StaggerFrom::Center);
///
/// // Apply easing to the stagger distribution
/// let stagger = Stagger::each(0.1).ease(eazy_data::Easing::OutQuadratic);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Stagger {
  /// Delay between each successive animation.
  each: f32,
  /// Direction of the stagger.
  from: StaggerFrom,
  /// Easing applied to the stagger distribution.
  ease: Option<Easing>,
  /// Total duration to distribute staggers across.
  /// If set, overrides `each` to fit all animations in this duration.
  total: Option<f32>,
}

impl Stagger {
  /// Create a stagger with the given delay between each animation.
  pub fn each(delay: f32) -> Self {
    Self {
      each: delay.max(0.0),
      from: StaggerFrom::default(),
      ease: None,
      total: None,
    }
  }

  /// Create a stagger that distributes animations across a total duration.
  ///
  /// The delay between each animation is calculated as `total / (count - 1)`.
  pub fn total(duration: f32) -> Self {
    Self {
      each: 0.0,
      from: StaggerFrom::default(),
      ease: None,
      total: Some(duration.max(0.0)),
    }
  }

  /// Set the stagger direction.
  pub fn from(mut self, from: StaggerFrom) -> Self {
    self.from = from;
    self
  }

  /// Apply an easing function to the stagger distribution.
  ///
  /// This affects how the delays are distributed, not the animations
  /// themselves.
  pub fn ease(mut self, easing: Easing) -> Self {
    self.ease = Some(easing);
    self
  }

  /// Get the delay for a specific index in a collection.
  ///
  /// # Parameters
  ///
  /// - `index`: The position of this animation (0-based)
  /// - `total`: Total number of animations
  ///
  /// # Returns
  ///
  /// The delay in seconds before this animation should start.
  pub fn delay_for(&self, index: usize, total: usize) -> f32 {
    if total == 0 {
      return 0.0;
    }

    if total == 1 {
      return 0.0;
    }

    // Get the base factor (0.0 to 1.0)
    let factor = self.from.index_factor(index, total);

    // Apply easing if present
    let eased_factor = match &self.ease {
      Some(easing) => easing.y(factor),
      None => factor,
    };

    // Calculate the delay
    let each_delay = match self.total {
      Some(t) => t / (total - 1) as f32,
      None => self.each,
    };

    eased_factor * each_delay * (total - 1) as f32
  }

  /// Get the total stagger duration for a collection.
  ///
  /// This is the delay of the last animation to start.
  pub fn total_stagger_duration(&self, count: usize) -> f32 {
    if count <= 1 {
      return 0.0;
    }

    match self.total {
      Some(t) => t,
      None => self.each * (count - 1) as f32,
    }
  }

  /// Get the each delay value.
  pub fn each_delay(&self) -> f32 {
    self.each
  }

  /// Get the stagger direction.
  pub fn direction(&self) -> StaggerFrom {
    self.from
  }
}

/// Calculate stagger delays for a collection of items.
///
/// Returns a vector of delays corresponding to each index.
pub fn calculate_stagger_delays(stagger: &Stagger, count: usize) -> Vec<f32> {
  (0..count)
    .map(|i| stagger.delay_for(i, count))
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_stagger_each() {
    let stagger = Stagger::each(0.1);

    assert_eq!(stagger.delay_for(0, 5), 0.0);
    assert!((stagger.delay_for(1, 5) - 0.1).abs() < 0.001);
    assert!((stagger.delay_for(2, 5) - 0.2).abs() < 0.001);
    assert!((stagger.delay_for(4, 5) - 0.4).abs() < 0.001);
  }

  #[test]
  fn test_stagger_from_end() {
    let stagger = Stagger::each(0.1).from(StaggerFrom::End);

    assert!((stagger.delay_for(0, 5) - 0.4).abs() < 0.001);
    assert!((stagger.delay_for(4, 5) - 0.0).abs() < 0.001);
  }

  #[test]
  fn test_stagger_from_center() {
    let stagger = Stagger::each(0.1).from(StaggerFrom::Center);

    // With 5 items (indices 0,1,2,3,4), center is 2
    // Delays should be: 2->0, 1&3->0.1, 0&4->0.2
    assert!((stagger.delay_for(2, 5) - 0.0).abs() < 0.001);
    assert!((stagger.delay_for(1, 5) - 0.2).abs() < 0.001);
    assert!((stagger.delay_for(3, 5) - 0.2).abs() < 0.001);
    assert!((stagger.delay_for(0, 5) - 0.4).abs() < 0.001);
    assert!((stagger.delay_for(4, 5) - 0.4).abs() < 0.001);
  }

  #[test]
  fn test_stagger_total() {
    let stagger = Stagger::total(1.0);

    // 5 items across 1.0 seconds = 0.25s between each
    assert_eq!(stagger.delay_for(0, 5), 0.0);
    assert!((stagger.delay_for(1, 5) - 0.25).abs() < 0.001);
    assert!((stagger.delay_for(4, 5) - 1.0).abs() < 0.001);
  }

  #[test]
  fn test_stagger_single_item() {
    let stagger = Stagger::each(0.1);

    assert_eq!(stagger.delay_for(0, 1), 0.0);
  }

  #[test]
  fn test_stagger_empty() {
    let stagger = Stagger::each(0.1);

    assert_eq!(stagger.delay_for(0, 0), 0.0);
  }

  #[test]
  fn test_total_stagger_duration() {
    let stagger = Stagger::each(0.1);

    assert_eq!(stagger.total_stagger_duration(5), 0.4);
    assert_eq!(stagger.total_stagger_duration(1), 0.0);
  }

  #[test]
  fn test_calculate_stagger_delays() {
    let stagger = Stagger::each(0.1);
    let delays = calculate_stagger_delays(&stagger, 3);

    assert_eq!(delays.len(), 3);
    assert_eq!(delays[0], 0.0);
    assert!((delays[1] - 0.1).abs() < 0.001);
    assert!((delays[2] - 0.2).abs() < 0.001);
  }
}
