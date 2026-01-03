//! Timeline for sequencing and controlling multiple animations.
//!
//! A [`Timeline`] orchestrates multiple tweens, controlling their playback
//! as a single unit with precise timing control.

use crate::callback::Callbacks;
use crate::control::Controllable;
use crate::control::Direction;
use crate::control::TweenState;
use crate::position::Position;
use crate::repeat::RepeatConfig;
use crate::stagger::Stagger;
use crate::tween::Tween;
use crate::value::Tweenable;

use rustc_hash::FxHashMap as HashMap;

/// A child animation within a timeline.
struct TimelineChild {
  /// The controllable animation.
  animation: Box<dyn Controllable>,
  /// Start time within the timeline.
  start_time: f32,
  /// Duration of this child.
  duration: f32,
}

impl TimelineChild {
  /// Get the end time of this child.
  fn end_time(&self) -> f32 {
    self.start_time + self.duration
  }

  /// Get the local time for this child given the timeline time.
  fn local_time(&self, timeline_time: f32) -> f32 {
    (timeline_time - self.start_time).max(0.0)
  }
}

/// A timeline that sequences and controls multiple animations.
///
/// # Examples
///
/// ```rust
/// use eazy_tweener::{Timeline, Tween, Position, Controllable};
///
/// let mut timeline = Timeline::builder()
///   .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
///   .push_at(
///     Tween::to(0.0_f32, 50.0).duration(0.5).build(),
///     Position::WithPrevious
///   )
///   .build();
///
/// timeline.play();
/// ```
pub struct Timeline {
  /// Child animations with their timing.
  children: Vec<TimelineChild>,
  /// Total duration (calculated from children).
  duration: f32,
  /// Current elapsed time.
  elapsed: f32,
  /// Current state.
  state: TweenState,
  /// Playback direction.
  direction: Direction,
  /// Time scale multiplier.
  time_scale: f32,
  /// Named position markers.
  labels: HashMap<String, f32>,
  /// Repeat configuration.
  repeat_config: RepeatConfig,
  /// Current repeat iteration.
  iteration: u32,
  /// Lifecycle callbacks.
  callbacks: Callbacks,
  /// Whether on_start has been fired.
  started: bool,
  /// Track which children have completed.
  children_started: Vec<bool>,
}

impl Timeline {
  /// Create a timeline builder for fluent construction.
  pub fn builder() -> TimelineBuilder {
    TimelineBuilder::new()
  }

  /// Create a timeline directly (use builder for fluent API).
  fn create() -> Self {
    Self {
      children: Vec::new(),
      duration: 0.0,
      elapsed: 0.0,
      state: TweenState::Idle,
      direction: Direction::Forward,
      time_scale: 1.0,
      labels: HashMap::default(),
      repeat_config: RepeatConfig::default(),
      iteration: 0,
      callbacks: Callbacks::default(),
      started: false,
      children_started: Vec::new(),
    }
  }

  /// Get the number of children in this timeline.
  pub fn len(&self) -> usize {
    self.children.len()
  }

  /// Check if the timeline is empty.
  pub fn is_empty(&self) -> bool {
    self.children.is_empty()
  }

  /// Get a label's time position.
  pub fn get_label(&self, name: &str) -> Option<f32> {
    self.labels.get(name).copied()
  }

  /// Get the current iteration.
  pub fn iteration(&self) -> u32 {
    self.iteration
  }

  /// Recalculate the total duration from children.
  fn recalculate_duration(&mut self) {
    self.duration = self
      .children
      .iter()
      .map(|c| c.end_time())
      .fold(0.0_f32, f32::max);
  }

  /// Add a child and return its end time.
  fn add_child(&mut self, animation: Box<dyn Controllable>, start_time: f32) {
    let duration = animation.duration();

    self.children.push(TimelineChild {
      animation,
      start_time,
      duration,
    });

    self.children_started.push(false);
    self.recalculate_duration();
  }

  /// Get the end time of the last child.
  fn last_child_end(&self) -> f32 {
    self.children.last().map(|c| c.end_time()).unwrap_or(0.0)
  }

  /// Get the start time of the last child.
  fn last_child_start(&self) -> f32 {
    self.children.last().map(|c| c.start_time).unwrap_or(0.0)
  }

  /// Handle timeline completion.
  fn handle_completion(&mut self) {
    if self.repeat_config.repeat.can_repeat(self.iteration) {
      self.iteration += 1;
      self.elapsed = 0.0;
      self.children_started.fill(false);

      // Reset all children
      for child in &mut self.children {
        child.animation.kill();
      }

      self.callbacks.fire_repeat();
    } else {
      self.state = TweenState::Complete;
      self.callbacks.fire_complete();
    }
  }
}

impl Default for Timeline {
  fn default() -> Self {
    Self::create()
  }
}

impl Controllable for Timeline {
  fn play(&mut self) {
    match self.state {
      TweenState::Idle => {
        self.state = TweenState::Playing;
        self.started = false;
      }
      TweenState::Paused => {
        self.state = TweenState::Playing;
      }
      TweenState::Complete => {
        self.restart();
      }
      TweenState::Playing => {}
    }
  }

  fn pause(&mut self) {
    if self.state == TweenState::Playing {
      self.state = TweenState::Paused;

      // Pause all active children
      for child in &mut self.children {
        if child.animation.state().is_active() {
          child.animation.pause();
        }
      }
    }
  }

  fn resume(&mut self) {
    if self.state == TweenState::Paused {
      self.state = TweenState::Playing;

      // Resume paused children
      for child in &mut self.children {
        if child.animation.state().is_paused() {
          child.animation.resume();
        }
      }
    }
  }

  fn reverse(&mut self) {
    self.direction.toggle();
  }

  fn restart(&mut self) {
    self.elapsed = 0.0;
    self.iteration = 0;
    self.started = false;
    self.children_started.fill(false);

    // Reset all children
    for child in &mut self.children {
      child.animation.kill();
    }

    self.state = TweenState::Playing;
  }

  fn seek(&mut self, time: f32) {
    self.elapsed = time.clamp(0.0, self.duration);

    // Update all children to match
    for (i, child) in self.children.iter_mut().enumerate() {
      if self.elapsed >= child.start_time {
        let local_time = child.local_time(self.elapsed);

        child.animation.seek(local_time.min(child.duration));

        if !self.children_started[i] {
          self.children_started[i] = true;
          child.animation.play();
        }
      } else {
        child.animation.kill();
        self.children_started[i] = false;
      }
    }
  }

  fn kill(&mut self) {
    self.state = TweenState::Idle;
    self.elapsed = 0.0;
    self.iteration = 0;
    self.started = false;
    self.children_started.fill(false);

    for child in &mut self.children {
      child.animation.kill();
    }
  }

  fn progress(&self) -> f32 {
    if self.duration == 0.0 {
      1.0
    } else {
      (self.elapsed / self.duration).clamp(0.0, 1.0)
    }
  }

  fn set_progress(&mut self, progress: f32) {
    self.seek(progress.clamp(0.0, 1.0) * self.duration);
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

    let scaled_delta = delta * self.time_scale * self.direction.multiplier();

    // Fire on_start once
    if !self.started {
      self.started = true;
      self.callbacks.fire_start();
    }

    // Update elapsed time
    self.elapsed = (self.elapsed + scaled_delta).max(0.0);

    // Fire on_update
    self.callbacks.fire_update();

    // Update children based on timeline time
    for (i, child) in self.children.iter_mut().enumerate() {
      // Check if child should start
      if !self.children_started[i] && self.elapsed >= child.start_time {
        self.children_started[i] = true;
        child.animation.play();
      }

      // Tick active children
      if self.children_started[i] && child.animation.state().is_active() {
        child.animation.tick(delta * self.time_scale);
      }
    }

    // Check completion
    if self.direction.is_forward() && self.elapsed >= self.duration {
      self.elapsed = self.duration;
      self.handle_completion();
    } else if self.direction.is_reverse() && self.elapsed <= 0.0 {
      self.elapsed = 0.0;
      self.handle_completion();
    }

    self.state.is_active()
  }
}

/// Builder for creating timelines with a fluent API.
pub struct TimelineBuilder {
  timeline: Timeline,
}

impl TimelineBuilder {
  /// Create a new timeline builder.
  pub fn new() -> Self {
    Self {
      timeline: Timeline::create(),
    }
  }

  /// Add a tween at the default position (after previous).
  pub fn push<T: Tweenable>(self, tween: Tween<T>) -> Self {
    self.push_at(tween, Position::AfterPrevious)
  }

  /// Add a tween at a specific position.
  pub fn push_at<T: Tweenable>(
    mut self,
    tween: Tween<T>,
    position: Position,
  ) -> Self {
    let previous_end = self.timeline.last_child_end();
    let previous_start = self.timeline.last_child_start();

    let start_time = position
      .resolve(
        previous_end,
        previous_start,
        self.timeline.duration,
        &self.timeline.labels,
      )
      .unwrap_or(previous_end);

    self.timeline.add_child(Box::new(tween), start_time);

    self
  }

  /// Add a controllable animation at the default position.
  pub fn add_controllable(self, animation: Box<dyn Controllable>) -> Self {
    self.add_controllable_at(animation, Position::AfterPrevious)
  }

  /// Add a controllable animation at a specific position.
  pub fn add_controllable_at(
    mut self,
    animation: Box<dyn Controllable>,
    position: Position,
  ) -> Self {
    let previous_end = self.timeline.last_child_end();
    let previous_start = self.timeline.last_child_start();

    let start_time = position
      .resolve(
        previous_end,
        previous_start,
        self.timeline.duration,
        &self.timeline.labels,
      )
      .unwrap_or(previous_end);

    self.timeline.add_child(animation, start_time);

    self
  }

  /// Add multiple tweens with staggered timing.
  pub fn push_staggered<T: Tweenable>(
    mut self,
    tweens: Vec<Tween<T>>,
    stagger: Stagger,
  ) -> Self {
    let base_start = self.timeline.last_child_end();
    let count = tweens.len();

    for (i, tween) in tweens.into_iter().enumerate() {
      let delay = stagger.delay_for(i, count);
      let start_time = base_start + delay;

      self.timeline.add_child(Box::new(tween), start_time);
    }

    self
  }

  /// Add a label at the current position (end of last child).
  pub fn label(mut self, name: impl Into<String>) -> Self {
    let time = self.timeline.last_child_end();

    self.timeline.labels.insert(name.into(), time);

    self
  }

  /// Add a label at a specific time.
  pub fn label_at(mut self, name: impl Into<String>, time: f32) -> Self {
    self.timeline.labels.insert(name.into(), time);
    self
  }

  /// Set the time scale.
  pub fn time_scale(mut self, scale: f32) -> Self {
    self.timeline.time_scale = scale.max(0.0);
    self
  }

  /// Set repeat configuration.
  pub fn repeat(mut self, config: RepeatConfig) -> Self {
    self.timeline.repeat_config = config;
    self
  }

  /// Set a sync callback for on_start.
  pub fn on_start<F>(mut self, f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    self.timeline.callbacks.on_start = Some(crate::callback::Callback::sync(f));
    self
  }

  /// Set a sync callback for on_update.
  pub fn on_update<F>(mut self, f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    self.timeline.callbacks.on_update =
      Some(crate::callback::Callback::sync(f));
    self
  }

  /// Set a sync callback for on_complete.
  pub fn on_complete<F>(mut self, f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    self.timeline.callbacks.on_complete =
      Some(crate::callback::Callback::sync(f));
    self
  }

  /// Build the timeline.
  pub fn build(self) -> Timeline {
    self.timeline
  }
}

impl Default for TimelineBuilder {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_sequential_timeline() {
    let mut timeline = Timeline::builder()
      .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
      .push(Tween::to(0.0_f32, 50.0).duration(0.5).build())
      .build();

    assert_eq!(timeline.duration(), 1.5);

    timeline.play();
    timeline.tick(0.5);

    assert!(timeline.elapsed() > 0.0);
  }

  #[test]
  fn test_parallel_timeline() {
    let timeline = Timeline::builder()
      .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
      .push_at(
        Tween::to(0.0_f32, 50.0).duration(0.5).build(),
        Position::WithPrevious,
      )
      .build();

    // Both start at 0, so duration is max of both = 1.0
    assert_eq!(timeline.duration(), 1.0);
  }

  #[test]
  fn test_overlapping_timeline() {
    let timeline = Timeline::builder()
      .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
      .push_at(
        Tween::to(0.0_f32, 50.0).duration(0.5).build(),
        Position::Relative(-0.3),
      )
      .build();

    // First ends at 1.0, second starts at 0.7, ends at 1.2
    assert!((timeline.duration() - 1.2).abs() < 0.001);
  }

  #[test]
  fn test_staggered_timeline() {
    let tweens: Vec<_> = (0..5)
      .map(|_| Tween::to(0.0_f32, 100.0).duration(0.5).build())
      .collect();

    let timeline = Timeline::builder()
      .push_staggered(tweens, Stagger::each(0.1))
      .build();

    // 5 tweens, each 0.5s, staggered by 0.1s
    // Last starts at 0.4, ends at 0.9
    assert!((timeline.duration() - 0.9).abs() < 0.001);
  }

  #[test]
  fn test_labels() {
    let timeline = Timeline::builder()
      .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
      .label("middle")
      .push(Tween::to(0.0_f32, 50.0).duration(0.5).build())
      .build();

    assert_eq!(timeline.get_label("middle"), Some(1.0));
  }

  #[test]
  fn test_seek() {
    let mut timeline = Timeline::builder()
      .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
      .push(Tween::to(0.0_f32, 50.0).duration(0.5).build())
      .build();

    timeline.seek(0.5);

    assert_eq!(timeline.elapsed(), 0.5);
  }

  #[test]
  fn test_timeline_completion() {
    let mut timeline = Timeline::builder()
      .push(Tween::to(0.0_f32, 100.0).duration(0.5).build())
      .build();

    timeline.play();
    timeline.tick(0.6);

    assert!(timeline.state().is_complete());
  }
}
