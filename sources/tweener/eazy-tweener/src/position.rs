//! Timeline positioning system for sequencing animations.
//!
//! The [`Position`] enum provides flexible ways to specify when a child
//! animation should start within a timeline.

use rustc_hash::FxHashMap as HashMap;

/// Specifies when a child animation starts within a timeline.
///
/// Similar to GSAP's position parameter, this allows for:
/// - Sequential animations (one after another)
/// - Parallel animations (starting together)
/// - Overlapping animations (one starts before another ends)
/// - Labeled positions (named time markers)
///
/// # Examples
///
/// ```rust
/// use eazy_tweener::Position;
///
/// // Add at 2 seconds from timeline start
/// let pos = Position::Absolute(2.0);
///
/// // Add 0.5 seconds after previous animation ends
/// let pos = Position::Relative(0.5);
///
/// // Overlap with previous animation by 0.3 seconds
/// let pos = Position::Relative(-0.3);
///
/// // Start at the same time as previous animation
/// let pos = Position::WithPrevious;
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Position {
  /// Start at an absolute time from the timeline's beginning.
  ///
  /// `Absolute(2.0)` means start at 2 seconds.
  Absolute(f32),

  /// Start relative to the end of the previous animation.
  ///
  /// `Relative(0.5)` means start 0.5 seconds after previous ends.
  /// `Relative(-0.3)` means start 0.3 seconds before previous ends
  /// (overlap).
  Relative(f32),

  /// Start at a named label position.
  ///
  /// Labels must be added to the timeline before referencing them.
  Label(String),

  /// Start at the same time the previous animation ends (sequential).
  ///
  /// This is the default behavior - equivalent to `Relative(0.0)`.
  #[default]
  AfterPrevious,

  /// Start at the same time as the previous animation (parallel).
  ///
  /// Useful for animating multiple properties simultaneously.
  WithPrevious,

  /// Start at the beginning of the timeline.
  Start,

  /// Start at the current end of the timeline.
  End,
}

impl Position {
  /// Calculate the absolute start time for this position.
  ///
  /// # Parameters
  ///
  /// - `previous_end`: End time of the previous child (0.0 if first child)
  /// - `previous_start`: Start time of the previous child (0.0 if first child)
  /// - `timeline_end`: Current total duration of the timeline
  /// - `labels`: Map of label names to their time positions
  ///
  /// # Returns
  ///
  /// The absolute start time in seconds, or `None` if label not found.
  pub fn resolve(
    &self,
    previous_end: f32,
    previous_start: f32,
    timeline_end: f32,
    labels: &HashMap<String, f32>,
  ) -> Option<f32> {
    let time = match self {
      Self::Absolute(t) => *t,
      Self::Relative(offset) => previous_end + offset,
      Self::Label(name) => *labels.get(name)?,
      Self::AfterPrevious => previous_end,
      Self::WithPrevious => previous_start,
      Self::Start => 0.0,
      Self::End => timeline_end,
    };

    Some(time.max(0.0))
  }

  /// Check if this position is absolute (not relative to other animations).
  pub fn is_absolute(&self) -> bool {
    matches!(self, Self::Absolute(_) | Self::Label(_) | Self::Start)
  }

  /// Check if this position is relative to the previous animation.
  pub fn is_relative(&self) -> bool {
    matches!(
      self,
      Self::Relative(_) | Self::AfterPrevious | Self::WithPrevious
    )
  }
}

impl From<f32> for Position {
  fn from(time: f32) -> Self {
    Self::Absolute(time)
  }
}

impl From<&str> for Position {
  fn from(s: &str) -> Self {
    // Parse GSAP-style position strings
    if s == "<" {
      return Self::WithPrevious;
    }

    if s == ">" {
      return Self::AfterPrevious;
    }

    if let Some(offset) = s.strip_prefix("+=")
      && let Ok(n) = offset.parse::<f32>()
    {
      return Self::Relative(n);
    }

    if let Some(offset) = s.strip_prefix("-=")
      && let Ok(n) = offset.parse::<f32>()
    {
      return Self::Relative(-n);
    }

    // Try parsing as absolute time
    if let Ok(n) = s.parse::<f32>() {
      return Self::Absolute(n);
    }

    // Otherwise treat as label
    Self::Label(s.to_string())
  }
}

impl From<String> for Position {
  fn from(s: String) -> Self {
    Self::from(s.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use rustc_hash::FxHashMap as HashMap;

  #[test]
  fn test_absolute_position() {
    let pos = Position::Absolute(2.0);
    let labels = HashMap::default();

    assert_eq!(pos.resolve(1.0, 0.0, 3.0, &labels), Some(2.0));
  }

  #[test]
  fn test_relative_position() {
    let labels = HashMap::default();

    let pos = Position::Relative(0.5);
    assert_eq!(pos.resolve(1.0, 0.0, 3.0, &labels), Some(1.5));

    let pos = Position::Relative(-0.3);
    assert_eq!(pos.resolve(1.0, 0.0, 3.0, &labels), Some(0.7));
  }

  #[test]
  fn test_after_previous() {
    let pos = Position::AfterPrevious;
    let labels = HashMap::default();

    assert_eq!(pos.resolve(1.5, 0.5, 3.0, &labels), Some(1.5));
  }

  #[test]
  fn test_with_previous() {
    let pos = Position::WithPrevious;
    let labels = HashMap::default();

    assert_eq!(pos.resolve(1.5, 0.5, 3.0, &labels), Some(0.5));
  }

  #[test]
  fn test_label_position() {
    let mut labels = HashMap::default();

    labels.insert("intro".to_string(), 1.0);

    let pos = Position::Label("intro".to_string());

    assert_eq!(pos.resolve(0.0, 0.0, 3.0, &labels), Some(1.0));

    let pos = Position::Label("missing".to_string());

    assert_eq!(pos.resolve(0.0, 0.0, 3.0, &labels), None);
  }

  #[test]
  fn test_from_string() {
    assert_eq!(Position::from("<"), Position::WithPrevious);
    assert_eq!(Position::from(">"), Position::AfterPrevious);
    assert_eq!(Position::from("+=0.5"), Position::Relative(0.5));
    assert_eq!(Position::from("-=0.3"), Position::Relative(-0.3));
    assert_eq!(Position::from("2.0"), Position::Absolute(2.0));
    assert_eq!(
      Position::from("myLabel"),
      Position::Label("myLabel".to_string())
    );
  }

  #[test]
  fn test_clamp_negative() {
    let pos = Position::Relative(-10.0);
    let labels = HashMap::default();

    // Should clamp to 0.0
    assert_eq!(pos.resolve(1.0, 0.0, 3.0, &labels), Some(0.0));
  }
}
