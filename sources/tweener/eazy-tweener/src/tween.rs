//! Core tween animation state machine.
//!
//! A [`Tween`] interpolates between two values over time using an easing
//! function from `eazy-data`.

use eazy_data::Curve;
use eazy_data::Easing;

use crate::callback::Callbacks;
use crate::control::Controllable;
use crate::control::Direction;
use crate::control::TweenState;
use crate::repeat::Repeat;
use crate::repeat::RepeatConfig;
use crate::value::Tweenable;

/// A tween that interpolates between two values of type `T`.
///
/// # Examples
///
/// ```rust
/// use eazy_tweener::{Tween, Controllable};
///
/// let mut tween = Tween::new(0.0_f32, 100.0, 1.0);
///
/// tween.play();
///
/// // In your update loop:
/// let value = tween.value();
/// tween.tick(0.016); // ~60 FPS
/// ```
#[derive(Debug, Clone)]
pub struct Tween<T: Tweenable> {
  /// Start value.
  from: T,
  /// End value.
  to: T,
  /// Duration in seconds.
  duration: f32,
  /// Current elapsed time.
  elapsed: f32,
  /// Initial delay before starting.
  delay: f32,
  /// Remaining delay (counts down).
  delay_remaining: f32,
  /// Easing function.
  easing: Easing,
  /// Current state.
  state: TweenState,
  /// Playback direction.
  direction: Direction,
  /// Repeat configuration.
  repeat_config: RepeatConfig,
  /// Current repeat iteration.
  iteration: u32,
  /// Time scale multiplier.
  time_scale: f32,
  /// Lifecycle callbacks.
  callbacks: Callbacks,
  /// Whether on_start has been fired for current play.
  started: bool,
}

impl<T: Tweenable> Tween<T> {
  /// Create a new tween from `from` to `to` over `duration` seconds.
  pub fn new(from: T, to: T, duration: f32) -> Self {
    Self {
      from,
      to,
      duration: duration.max(0.0),
      elapsed: 0.0,
      delay: 0.0,
      delay_remaining: 0.0,
      easing: Easing::Linear,
      state: TweenState::Idle,
      direction: Direction::Forward,
      repeat_config: RepeatConfig::default(),
      iteration: 0,
      time_scale: 1.0,
      callbacks: Callbacks::default(),
      started: false,
    }
  }

  /// Create a tween that animates TO the target value.
  ///
  /// The `from` value is provided, animating toward `to`.
  pub fn to(from: T, to: T) -> TweenBuilder<T> {
    TweenBuilder::new(from, to)
  }

  /// Create a tween that animates FROM the source value.
  ///
  /// The animation goes from `from` to `to`.
  pub fn from(from: T, to: T) -> TweenBuilder<T> {
    TweenBuilder::new(from, to)
  }

  /// Create a tween with explicit from and to values.
  pub fn from_to(from: T, to: T) -> TweenBuilder<T> {
    TweenBuilder::new(from, to)
  }

  // --- Accessors ---

  /// Get the start value.
  pub fn from_value(&self) -> T {
    self.from
  }

  /// Get the end value.
  pub fn to_value(&self) -> T {
    self.to
  }

  /// Get the current interpolated value.
  ///
  /// This applies the easing function to the current progress.
  pub fn value(&self) -> T {
    let progress = self.eased_progress();

    // When reversed via yoyo, we flip the interpolation
    if self.repeat_config.should_reverse(self.iteration) {
      self.to.lerp(self.from, progress)
    } else {
      self.from.lerp(self.to, progress)
    }
  }

  /// Get the raw progress [0, 1] without easing applied.
  pub fn raw_progress(&self) -> f32 {
    if self.duration == 0.0 {
      1.0
    } else {
      (self.elapsed / self.duration).clamp(0.0, 1.0)
    }
  }

  /// Get the eased progress using the configured easing function.
  pub fn eased_progress(&self) -> f32 {
    let raw = self.raw_progress();

    // Apply direction
    let directed = if self.direction.is_reverse() {
      1.0 - raw
    } else {
      raw
    };

    self.easing.y(directed)
  }

  /// Get the current iteration count.
  pub fn iteration(&self) -> u32 {
    self.iteration
  }

  /// Check if currently in a yoyo reverse phase.
  pub fn is_yoyo_reversed(&self) -> bool {
    self.repeat_config.should_reverse(self.iteration)
  }

  // --- Configuration ---

  /// Set the easing function.
  pub fn set_easing(&mut self, easing: Easing) {
    self.easing = easing;
  }

  /// Set the repeat configuration.
  pub fn set_repeat(&mut self, config: RepeatConfig) {
    self.repeat_config = config;
  }

  /// Set the initial delay.
  pub fn set_delay(&mut self, delay: f32) {
    self.delay = delay.max(0.0);

    if self.state == TweenState::Idle {
      self.delay_remaining = self.delay;
    }
  }

  /// Set the callbacks.
  pub fn set_callbacks(&mut self, callbacks: Callbacks) {
    self.callbacks = callbacks;
  }

  // --- Internal ---

  fn handle_completion(&mut self) {
    // Check if we should repeat
    if self.repeat_config.repeat.can_repeat(self.iteration) {
      self.iteration += 1;
      self.elapsed = 0.0;
      self.delay_remaining = self.repeat_config.delay;
      self.callbacks.fire_repeat();
    } else {
      self.state = TweenState::Complete;
      self.callbacks.fire_complete();
    }
  }
}

impl<T: Tweenable> Controllable for Tween<T> {
  fn play(&mut self) {
    match self.state {
      TweenState::Idle => {
        self.state = TweenState::Playing;
        self.delay_remaining = self.delay;
        self.started = false;
      }
      TweenState::Paused => {
        self.state = TweenState::Playing;
      }
      TweenState::Complete => {
        // Restart from beginning
        self.restart();
      }
      TweenState::Playing => {
        // Already playing
      }
    }
  }

  fn pause(&mut self) {
    if self.state == TweenState::Playing {
      self.state = TweenState::Paused;
    }
  }

  fn resume(&mut self) {
    if self.state == TweenState::Paused {
      self.state = TweenState::Playing;
    }
  }

  fn reverse(&mut self) {
    self.direction.toggle();
  }

  fn restart(&mut self) {
    self.elapsed = 0.0;
    self.iteration = 0;
    self.delay_remaining = self.delay;
    self.started = false;
    self.state = TweenState::Playing;
  }

  fn seek(&mut self, time: f32) {
    self.elapsed = time.clamp(0.0, self.duration);
  }

  fn kill(&mut self) {
    self.state = TweenState::Idle;
    self.elapsed = 0.0;
    self.iteration = 0;
    self.delay_remaining = self.delay;
    self.started = false;
  }

  fn progress(&self) -> f32 {
    self.raw_progress()
  }

  fn set_progress(&mut self, progress: f32) {
    self.elapsed = progress.clamp(0.0, 1.0) * self.duration;
  }

  fn duration(&self) -> f32 {
    self.duration
  }

  fn elapsed(&self) -> f32 {
    self.elapsed
  }

  fn state(&self) -> TweenState {
    self.state
  }

  fn direction(&self) -> Direction {
    self.direction
  }

  fn time_scale(&self) -> f32 {
    self.time_scale
  }

  fn set_time_scale(&mut self, scale: f32) {
    self.time_scale = scale.max(0.0);
  }

  fn tick(&mut self, delta: f32) -> bool {
    if self.state != TweenState::Playing {
      return self.state.is_active();
    }

    let scaled_delta = delta * self.time_scale;

    // Handle initial delay
    if self.delay_remaining > 0.0 {
      self.delay_remaining -= scaled_delta;

      if self.delay_remaining > 0.0 {
        return true;
      }

      // Delay complete, apply overflow to elapsed
      let overflow = -self.delay_remaining;

      self.delay_remaining = 0.0;
      self.elapsed += overflow;
    } else {
      self.elapsed += scaled_delta;
    }

    // Fire on_start once per play cycle
    if !self.started {
      self.started = true;
      self.callbacks.fire_start();
    }

    // Fire on_update
    self.callbacks.fire_update();

    // Check completion
    if self.elapsed >= self.duration {
      self.elapsed = self.duration;
      self.handle_completion();
    }

    self.state.is_active()
  }
}

/// Builder for creating tweens with fluent API.
#[derive(Debug, Clone)]
pub struct TweenBuilder<T: Tweenable> {
  tween: Tween<T>,
}

impl<T: Tweenable> TweenBuilder<T> {
  /// Create a new builder.
  pub fn new(from: T, to: T) -> Self {
    Self {
      tween: Tween::new(from, to, 1.0),
    }
  }

  /// Set the duration in seconds.
  pub fn duration(mut self, secs: f32) -> Self {
    self.tween.duration = secs.max(0.0);
    self
  }

  /// Set the easing function.
  pub fn easing(mut self, easing: Easing) -> Self {
    self.tween.easing = easing;
    self
  }

  /// Set repeat count.
  pub fn repeat(mut self, repeat: impl Into<Repeat>) -> Self {
    self.tween.repeat_config.repeat = repeat.into();
    self
  }

  /// Enable yoyo mode (alternate direction on each repeat).
  pub fn yoyo(mut self, enabled: bool) -> Self {
    self.tween.repeat_config.yoyo = enabled;
    self
  }

  /// Set delay between repeats.
  pub fn repeat_delay(mut self, delay: f32) -> Self {
    self.tween.repeat_config.delay = delay;
    self
  }

  /// Set initial delay before the animation starts.
  pub fn delay(mut self, delay: f32) -> Self {
    self.tween.delay = delay.max(0.0);
    self.tween.delay_remaining = self.tween.delay;
    self
  }

  /// Set the time scale multiplier.
  pub fn time_scale(mut self, scale: f32) -> Self {
    self.tween.time_scale = scale.max(0.0);
    self
  }

  /// Set a sync callback for on_start.
  pub fn on_start<F>(mut self, f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    self.tween.callbacks.on_start =
      Some(crate::callback::Callback::sync(f));
    self
  }

  /// Set a sync callback for on_update.
  pub fn on_update<F>(mut self, f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    self.tween.callbacks.on_update =
      Some(crate::callback::Callback::sync(f));
    self
  }

  /// Set a sync callback for on_complete.
  pub fn on_complete<F>(mut self, f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    self.tween.callbacks.on_complete =
      Some(crate::callback::Callback::sync(f));
    self
  }

  /// Set a sync callback for on_repeat.
  pub fn on_repeat<F>(mut self, f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    self.tween.callbacks.on_repeat =
      Some(crate::callback::Callback::sync(f));
    self
  }

  /// Build the tween.
  pub fn build(self) -> Tween<T> {
    self.tween
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_tween() {
    let mut tween = Tween::new(0.0_f32, 100.0, 1.0);

    tween.play();

    assert_eq!(tween.value(), 0.0);

    tween.tick(0.5);
    assert!((tween.value() - 50.0).abs() < 0.001);

    tween.tick(0.5);
    assert_eq!(tween.value(), 100.0);
    assert!(tween.state().is_complete());
  }

  #[test]
  fn test_tween_builder() {
    let tween = Tween::to(0.0_f32, 100.0)
      .duration(2.0)
      .easing(Easing::InOutQuadratic)
      .delay(0.5)
      .build();

    assert_eq!(tween.duration(), 2.0);
    assert_eq!(tween.delay, 0.5);
  }

  #[test]
  fn test_repeat() {
    let mut tween = Tween::to(0.0_f32, 100.0)
      .duration(1.0)
      .repeat(2u32)
      .build();

    tween.play();
    tween.tick(1.0); // Complete first
    assert_eq!(tween.iteration(), 1);
    assert!(tween.state().is_active());

    tween.tick(1.0); // Complete second
    assert_eq!(tween.iteration(), 2);
    assert!(tween.state().is_active());

    tween.tick(1.0); // Complete third (final)
    assert!(tween.state().is_complete());
  }

  #[test]
  fn test_yoyo() {
    let mut tween = Tween::to(0.0_f32, 100.0)
      .duration(1.0)
      .repeat(1u32)
      .yoyo(true)
      .build();

    tween.play();

    // First iteration: 0 -> 100
    tween.tick(0.5);
    assert!((tween.value() - 50.0).abs() < 0.001);

    tween.tick(0.5);
    assert_eq!(tween.iteration(), 1);

    // Second iteration (yoyo): 100 -> 0
    tween.tick(0.5);
    assert!((tween.value() - 50.0).abs() < 0.001);
  }

  #[test]
  fn test_delay() {
    let mut tween = Tween::to(0.0_f32, 100.0)
      .duration(1.0)
      .delay(0.5)
      .build();

    tween.play();

    tween.tick(0.25);
    assert_eq!(tween.elapsed(), 0.0); // Still in delay

    tween.tick(0.25);
    assert_eq!(tween.elapsed(), 0.0); // Still in delay

    tween.tick(0.25);
    // Delay complete, 0.25s into animation
    assert!(tween.elapsed() > 0.0);
  }

  #[test]
  fn test_time_scale() {
    let mut tween = Tween::to(0.0_f32, 100.0)
      .duration(1.0)
      .time_scale(2.0)
      .build();

    tween.play();
    tween.tick(0.25); // At 2x speed, this is 0.5s of animation

    assert!((tween.elapsed() - 0.5).abs() < 0.001);
  }

  #[test]
  fn test_seek() {
    let mut tween = Tween::new(0.0_f32, 100.0, 1.0);

    tween.seek(0.75);
    assert!((tween.value() - 75.0).abs() < 0.001);
  }

  #[test]
  fn test_array_tween() {
    let mut tween = Tween::new([0.0_f32, 0.0, 0.0], [100.0, 200.0, 300.0], 1.0);

    tween.play();
    tween.tick(0.5);

    let value = tween.value();

    assert!((value[0] - 50.0).abs() < 0.001);
    assert!((value[1] - 100.0).abs() < 0.001);
    assert!((value[2] - 150.0).abs() < 0.001);
  }
}
