//! Keyframe track for sampling interpolated values over time.
//!
//! A [`KeyframeTrack`] holds a sequence of keyframes and provides
//! sampling functionality to get interpolated values at any time.

use crate::keyframe::Keyframe;

use eazy_data::Curve;
use eazy_data::Easing;
use eazy_tweener::Tweenable;

/// A track containing keyframes that can be sampled at any time.
///
/// # Type Parameters
///
/// - `T`: The value type, must implement [`Tweenable`].
///
/// # Examples
///
/// ```rust
/// use eazy_keyframes::{KeyframeTrack, Keyframe};
/// use eazy_data::Easing;
///
/// let track = KeyframeTrack::new()
///   .keyframe(0.0, 0.0_f32)
///   .keyframe_eased(0.5, 100.0, Easing::OutBounce)
///   .keyframe(1.0, 50.0);
///
/// let value = track.sample(0.25);
/// ```
#[derive(Debug, Clone)]
pub struct KeyframeTrack<T: Tweenable> {
  /// Sorted list of keyframes.
  keyframes: Vec<Keyframe<T>>,
  /// Default easing for keyframes without explicit easing.
  default_easing: Easing,
}

impl<T: Tweenable> KeyframeTrack<T> {
  /// Create a new empty keyframe track.
  pub fn new() -> Self {
    Self {
      keyframes: Vec::new(),
      default_easing: Easing::Linear,
    }
  }

  /// Create a track with a specific default easing.
  pub fn with_default_easing(easing: Easing) -> Self {
    Self {
      keyframes: Vec::new(),
      default_easing: easing,
    }
  }

  /// Add a keyframe to the track.
  pub fn add(&mut self, keyframe: Keyframe<T>) {
    self.keyframes.push(keyframe);
    self.sort();
  }

  /// Add a keyframe (builder pattern).
  pub fn keyframe(mut self, time: f32, value: T) -> Self {
    self.keyframes.push(Keyframe::new(time, value));
    self.sort();
    self
  }

  /// Add a keyframe with easing (builder pattern).
  pub fn keyframe_eased(mut self, time: f32, value: T, easing: Easing) -> Self {
    self
      .keyframes
      .push(Keyframe::new(time, value).with_easing(easing));
    self.sort();
    self
  }

  /// Sort keyframes by time.
  fn sort(&mut self) {
    self
      .keyframes
      .sort_by(|a, b| a.time().partial_cmp(&b.time()).unwrap());
  }

  /// Get the number of keyframes.
  pub fn len(&self) -> usize {
    self.keyframes.len()
  }

  /// Check if the track is empty.
  pub fn is_empty(&self) -> bool {
    self.keyframes.is_empty()
  }

  /// Get the duration of the track (time of last keyframe).
  pub fn duration(&self) -> f32 {
    self.keyframes.last().map(|kf| kf.time()).unwrap_or(0.0)
  }

  /// Get the start time (time of first keyframe).
  pub fn start_time(&self) -> f32 {
    self.keyframes.first().map(|kf| kf.time()).unwrap_or(0.0)
  }

  /// Get a keyframe by index.
  pub fn get(&self, index: usize) -> Option<&Keyframe<T>> {
    self.keyframes.get(index)
  }

  /// Sample the track at a given time.
  ///
  /// Returns the interpolated value between keyframes.
  /// Times before the first keyframe return the first value.
  /// Times after the last keyframe return the last value.
  pub fn sample(&self, time: f32) -> T {
    match self.keyframes.len() {
      0 => panic!("Cannot sample empty keyframe track"),
      1 => self.keyframes[0].value(),
      _ => self.sample_between(time),
    }
  }

  /// Sample with clamping to track bounds.
  pub fn sample_clamped(&self, time: f32) -> T {
    let clamped = time.clamp(self.start_time(), self.duration());

    self.sample(clamped)
  }

  /// Find the keyframe pair surrounding the given time.
  fn find_keyframe_pair(&self, time: f32) -> (usize, usize) {
    // Find the first keyframe with time > target
    let next_idx = self
      .keyframes
      .iter()
      .position(|kf| kf.time() > time)
      .unwrap_or(self.keyframes.len());

    if next_idx == 0 {
      (0, 0)
    } else if next_idx >= self.keyframes.len() {
      let last = self.keyframes.len() - 1;

      (last, last)
    } else {
      (next_idx - 1, next_idx)
    }
  }

  /// Sample between keyframes.
  fn sample_between(&self, time: f32) -> T {
    let (prev_idx, next_idx) = self.find_keyframe_pair(time);

    // Same keyframe (at bounds)
    if prev_idx == next_idx {
      return self.keyframes[prev_idx].value();
    }

    let prev = &self.keyframes[prev_idx];
    let next = &self.keyframes[next_idx];

    // Calculate local progress between keyframes
    let segment_duration = next.time() - prev.time();

    if segment_duration == 0.0 {
      return next.value();
    }

    let local_progress = (time - prev.time()) / segment_duration;

    // Apply easing (use next keyframe's easing, as it defines how we arrive)
    let easing = next.easing().unwrap_or(&self.default_easing);
    let eased_progress = easing.y(local_progress);

    // Interpolate
    prev.value().lerp(next.value(), eased_progress)
  }

  /// Iterate over keyframes.
  pub fn iter(&self) -> impl Iterator<Item = &Keyframe<T>> {
    self.keyframes.iter()
  }
}

impl<T: Tweenable> Default for KeyframeTrack<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Tweenable> FromIterator<Keyframe<T>> for KeyframeTrack<T> {
  fn from_iter<I: IntoIterator<Item = Keyframe<T>>>(iter: I) -> Self {
    let mut track = Self::new();

    for kf in iter {
      track.keyframes.push(kf);
    }

    track.sort();
    track
  }
}

impl<T: Tweenable> From<Vec<Keyframe<T>>> for KeyframeTrack<T> {
  fn from(keyframes: Vec<Keyframe<T>>) -> Self {
    let mut track = Self {
      keyframes,
      default_easing: Easing::Linear,
    };

    track.sort();
    track
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_sample_linear() {
    let track = KeyframeTrack::new()
      .keyframe(0.0, 0.0_f32)
      .keyframe(1.0, 100.0);

    assert_eq!(track.sample(0.0), 0.0);
    assert!((track.sample(0.5) - 50.0).abs() < 0.001);
    assert_eq!(track.sample(1.0), 100.0);
  }

  #[test]
  fn test_sample_multiple_keyframes() {
    let track = KeyframeTrack::new()
      .keyframe(0.0, 0.0_f32)
      .keyframe(0.5, 100.0)
      .keyframe(1.0, 50.0);

    assert_eq!(track.sample(0.0), 0.0);
    assert!((track.sample(0.25) - 50.0).abs() < 0.001);
    assert_eq!(track.sample(0.5), 100.0);
    assert!((track.sample(0.75) - 75.0).abs() < 0.001);
    assert_eq!(track.sample(1.0), 50.0);
  }

  #[test]
  fn test_sample_before_first() {
    let track = KeyframeTrack::new()
      .keyframe(0.5, 100.0_f32)
      .keyframe(1.0, 200.0);

    // Before first keyframe should return first value
    assert_eq!(track.sample(0.0), 100.0);
    assert_eq!(track.sample(0.25), 100.0);
  }

  #[test]
  fn test_sample_after_last() {
    let track = KeyframeTrack::new()
      .keyframe(0.0, 0.0_f32)
      .keyframe(0.5, 100.0);

    // After last keyframe should return last value
    assert_eq!(track.sample(0.75), 100.0);
    assert_eq!(track.sample(1.0), 100.0);
  }

  #[test]
  fn test_sample_single_keyframe() {
    let track = KeyframeTrack::new().keyframe(0.5, 100.0_f32);

    assert_eq!(track.sample(0.0), 100.0);
    assert_eq!(track.sample(0.5), 100.0);
    assert_eq!(track.sample(1.0), 100.0);
  }

  #[test]
  fn test_sample_array() {
    let track = KeyframeTrack::new()
      .keyframe(0.0, [0.0_f32, 0.0, 0.0])
      .keyframe(1.0, [100.0, 200.0, 300.0]);

    let mid = track.sample(0.5);

    assert!((mid[0] - 50.0).abs() < 0.001);
    assert!((mid[1] - 100.0).abs() < 0.001);
    assert!((mid[2] - 150.0).abs() < 0.001);
  }

  #[test]
  fn test_duration() {
    let track = KeyframeTrack::new()
      .keyframe(0.0, 0.0_f32)
      .keyframe(2.5, 100.0);

    assert_eq!(track.duration(), 2.5);
  }

  #[test]
  fn test_keyframe_ordering() {
    // Add keyframes out of order
    let track = KeyframeTrack::new()
      .keyframe(1.0, 100.0_f32)
      .keyframe(0.0, 0.0)
      .keyframe(0.5, 50.0);

    // Should be sorted
    assert_eq!(track.get(0).unwrap().time(), 0.0);
    assert_eq!(track.get(1).unwrap().time(), 0.5);
    assert_eq!(track.get(2).unwrap().time(), 1.0);
  }

  #[test]
  fn test_from_iterator() {
    let keyframes =
      vec![Keyframe::new(1.0, 100.0_f32), Keyframe::new(0.0, 0.0)];

    let track: KeyframeTrack<f32> = keyframes.into_iter().collect();

    assert_eq!(track.len(), 2);
    assert_eq!(track.get(0).unwrap().time(), 0.0);
  }

  #[test]
  fn test_from_vec() {
    let keyframes =
      vec![Keyframe::new(1.0, 100.0_f32), Keyframe::new(0.0, 0.0)];

    let track = KeyframeTrack::from(keyframes);

    assert_eq!(track.len(), 2);
    assert_eq!(track.get(0).unwrap().time(), 0.0);
  }

  #[test]
  fn test_keyframes_macro() {
    use crate::keyframes;

    let track = keyframes![
      (0.0, 0.0_f32),
      (0.5, 100.0, Easing::InQuadratic),
      (1.0, 50.0)
    ];

    assert_eq!(track.len(), 3);
    assert_eq!(track.sample(0.0), 0.0);
    assert_eq!(track.sample(1.0), 50.0);

    // At 0.25 (midpoint of first segment, InQuadratic on second keyframe).
    // InQuadratic(0.5) = 0.25, so value = 0.0 + (100.0 - 0.0) * 0.25 = 25.0.
    let val = track.sample(0.25);

    assert_eq!(val, 25.0);
  }

  #[test]
  fn test_keyframes_macro_array() {
    use crate::keyframes;

    let track = keyframes![(0.0, [0.0_f32, 0.0]), (1.0, [100.0, 200.0])];

    let mid = track.sample(0.5);

    assert_eq!(mid, [50.0, 100.0]);
  }
}
