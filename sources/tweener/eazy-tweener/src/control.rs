//! Playback control interface for animations.
//!
//! Defines the [`Controllable`] trait shared by [`Tween`] and [`Timeline`],
//! providing a unified interface for playback control.

/// The current state of an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TweenState {
  /// Animation has not started or has been reset.
  #[default]
  Idle,
  /// Animation is actively playing.
  Playing,
  /// Animation is paused and can be resumed.
  Paused,
  /// Animation has finished playing.
  Complete,
}

impl TweenState {
  /// Check if the animation is currently active (playing).
  pub fn is_active(&self) -> bool {
    matches!(self, Self::Playing)
  }

  /// Check if the animation has completed.
  pub fn is_complete(&self) -> bool {
    matches!(self, Self::Complete)
  }

  /// Check if the animation is paused.
  pub fn is_paused(&self) -> bool {
    matches!(self, Self::Paused)
  }

  /// Check if the animation is idle (not started or reset).
  pub fn is_idle(&self) -> bool {
    matches!(self, Self::Idle)
  }
}

/// The playback direction of an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
  /// Playing forward (from → to).
  #[default]
  Forward,
  /// Playing in reverse (to → from).
  Reverse,
}

impl Direction {
  /// Toggle the direction.
  pub fn toggle(&mut self) {
    *self = match self {
      Self::Forward => Self::Reverse,
      Self::Reverse => Self::Forward,
    };
  }

  /// Check if playing forward.
  pub fn is_forward(&self) -> bool {
    matches!(self, Self::Forward)
  }

  /// Check if playing in reverse.
  pub fn is_reverse(&self) -> bool {
    matches!(self, Self::Reverse)
  }

  /// Get the time direction multiplier.
  ///
  /// Returns 1.0 for forward, -1.0 for reverse.
  pub fn multiplier(&self) -> f32 {
    match self {
      Self::Forward => 1.0,
      Self::Reverse => -1.0,
    }
  }
}

impl std::ops::Not for Direction {
  type Output = Self;

  fn not(self) -> Self::Output {
    match self {
      Self::Forward => Self::Reverse,
      Self::Reverse => Self::Forward,
    }
  }
}

/// Shared interface for controllable animations.
///
/// Both [`Tween`] and [`Timeline`] implement this trait, allowing
/// unified control over any animation type.
pub trait Controllable: Send + Sync {
  /// Start or resume playback.
  fn play(&mut self);

  /// Pause playback at the current position.
  fn pause(&mut self);

  /// Resume from a paused state.
  fn resume(&mut self);

  /// Toggle the playback direction.
  fn reverse(&mut self);

  /// Reset to the beginning and start playing.
  fn restart(&mut self);

  /// Jump to a specific time position.
  fn seek(&mut self, time: f32);

  /// Stop and reset the animation.
  fn kill(&mut self);

  /// Get the current progress as a normalized value [0, 1].
  fn progress(&self) -> f32;

  /// Set the progress directly [0, 1].
  fn set_progress(&mut self, progress: f32);

  /// Get the total duration in seconds.
  fn duration(&self) -> f32;

  /// Get the current elapsed time.
  fn elapsed(&self) -> f32;

  /// Get the current state.
  fn state(&self) -> TweenState;

  /// Get the current playback direction.
  fn direction(&self) -> Direction;

  /// Get the time scale multiplier.
  fn time_scale(&self) -> f32;

  /// Set the time scale multiplier.
  fn set_time_scale(&mut self, scale: f32);

  /// Advance the animation by delta time.
  ///
  /// Returns `true` if the animation is still active after the tick.
  fn tick(&mut self, delta: f32) -> bool;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_tween_state() {
    assert!(TweenState::Playing.is_active());
    assert!(!TweenState::Paused.is_active());
    assert!(TweenState::Complete.is_complete());
    assert!(TweenState::Idle.is_idle());
  }

  #[test]
  fn test_direction_toggle() {
    let mut dir = Direction::Forward;

    dir.toggle();
    assert!(dir.is_reverse());

    dir.toggle();
    assert!(dir.is_forward());
  }

  #[test]
  fn test_direction_multiplier() {
    assert_eq!(Direction::Forward.multiplier(), 1.0);
    assert_eq!(Direction::Reverse.multiplier(), -1.0);
  }

  #[test]
  fn test_direction_not() {
    assert_eq!(!Direction::Forward, Direction::Reverse);
    assert_eq!(!Direction::Reverse, Direction::Forward);
  }
}
