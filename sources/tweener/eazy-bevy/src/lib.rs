//! eazy-bevy — Bevy plugin for eazy animation library.
//!
//! Integrates eazy-tweener's `Tween` and `Timeline` with Bevy's ECS.
//!
//! # Design Philosophy
//!
//! eazy returns values, you apply them. This matches Bevy's ECS philosophy
//! where systems query components and mutate them explicitly.
//!
//! # Examples
//!
//! ## Basic Tween
//!
//! ```rust,ignore
//! use bevy::prelude::*;
//! use eazy_bevy::{EazyPlugin, Animator, AnimatorBundle};
//! use eazy_tweener::{Tween, Tweenable};
//! use eazy_core::Easing;
//!
//! fn setup(mut commands: Commands) {
//!   commands.spawn((
//!     Transform::default(),
//!     Animator::new(
//!       Tween::to(0.0_f32, 100.0)
//!         .duration(1.0)
//!         .easing(Easing::OutBounce)
//!         .build()
//!     ),
//!   ));
//! }
//!
//! fn animate(mut query: Query<(&mut Transform, &Animator<f32>)>) {
//!   for (mut transform, animator) in &mut query {
//!     transform.translation.y = animator.value();
//!   }
//! }
//! ```

use bevy::prelude::*;

/// Bevy plugin that registers animation systems.
///
/// Add this to your app to enable automatic ticking of animators.
///
/// ```rust,ignore
/// App::new()
///   .add_plugins(DefaultPlugins)
///   .add_plugins(EazyPlugin)
///   .run();
/// ```
pub struct EazyPlugin;

impl Plugin for EazyPlugin {
  fn build(&self, app: &mut App) {
    app.add_systems(
      Update,
      (
        tick_animators::<f32>,
        tick_animators::<f64>,
        tick_animators::<[f32; 2]>,
        tick_animators::<[f32; 3]>,
        tick_animators::<[f32; 4]>,
        tick_animators::<(f32, f32)>,
        tick_animators::<(f32, f32, f32)>,
        tick_timeline_animators,
      ),
    );
  }
}

/// Extension plugin for custom Tweenable types.
///
/// Use this to register tick systems for your own types.
///
/// ```rust,ignore
/// #[derive(Clone, Copy, Tweenable)]
/// struct MyType { x: f32, y: f32 }
///
/// app.add_plugins(EazyPlugin)
///    .add_plugins(EazyAnimatorPlugin::<MyType>::default());
/// ```
#[derive(Default)]
pub struct EazyAnimatorPlugin<T: Tweenable>(std::marker::PhantomData<T>);

impl<T: Tweenable> Plugin for EazyAnimatorPlugin<T> {
  fn build(&self, app: &mut App) {
    app.add_systems(Update, tick_animators::<T>);
  }
}

// ============================================================================
// Animator Component
// ============================================================================

/// Component that wraps a `Tween` and tracks its state.
///
/// The animator is automatically ticked by `EazyPlugin`. Query for
/// `Animator<T>` and call `.value()` to get the current animated value.
///
/// # Examples
///
/// ```rust,ignore
/// // Spawn an entity with an animator
/// commands.spawn((
///   Transform::default(),
///   Animator::new(
///     Tween::to(0.0_f32, 100.0)
///       .duration(2.0)
///       .easing(Easing::OutElastic)
///       .build()
///   ).playing(),  // Start playing immediately
/// ));
///
/// // In your system, read the animated value
/// fn animate(mut query: Query<(&mut Transform, &Animator<f32>)>) {
///   for (mut transform, animator) in &mut query {
///     transform.translation.y = animator.value();
///   }
/// }
/// ```
#[derive(Component)]
pub struct Animator<T: Tweenable> {
  tween: Tween<T>,
}

impl<T: Tweenable> Animator<T> {
  /// Create a new animator from a tween.
  #[inline(always)]
  pub fn new(tween: Tween<T>) -> Self {
    Self { tween }
  }

  /// Start playing immediately (builder pattern).
  #[inline(always)]
  pub fn playing(mut self) -> Self {
    self.tween.play();
    self
  }

  /// Get the current animated value.
  #[inline(always)]
  pub fn value(&self) -> T {
    self.tween.value()
  }

  /// Check if the animation is complete.
  #[inline(always)]
  pub fn is_complete(&self) -> bool {
    self.tween.state().is_complete()
  }

  /// Check if the animation is playing.
  #[inline(always)]
  pub fn is_playing(&self) -> bool {
    self.tween.state().is_active()
  }

  /// Get the progress (0.0 to 1.0).
  #[inline(always)]
  pub fn progress(&self) -> f32 {
    self.tween.progress()
  }

  /// Play the animation.
  #[inline(always)]
  pub fn play(&mut self) {
    self.tween.play();
  }

  /// Pause the animation.
  #[inline(always)]
  pub fn pause(&mut self) {
    self.tween.pause();
  }

  /// Resume the animation.
  #[inline(always)]
  pub fn resume(&mut self) {
    self.tween.resume();
  }

  /// Reverse the animation direction.
  #[inline(always)]
  pub fn reverse(&mut self) {
    self.tween.reverse();
  }

  /// Restart the animation from the beginning.
  #[inline(always)]
  pub fn restart(&mut self) {
    self.tween.restart();
  }

  /// Seek to a specific time.
  #[inline(always)]
  pub fn seek(&mut self, time: f32) {
    self.tween.seek(time);
  }

  /// Get the underlying tween.
  #[inline(always)]
  pub fn tween(&self) -> &Tween<T> {
    &self.tween
  }

  /// Get mutable access to the underlying tween.
  #[inline(always)]
  pub fn tween_mut(&mut self) -> &mut Tween<T> {
    &mut self.tween
  }
}

// ============================================================================
// Timeline Animator
// ============================================================================

/// Component that wraps a `Timeline` for complex sequenced animations.
///
/// Unlike `Animator<T>`, timeline doesn't output a single value.
/// You typically use it for coordinating multiple animations.
#[derive(Component)]
pub struct TimelineAnimator {
  timeline: Timeline,
}

impl TimelineAnimator {
  /// Create a new timeline animator.
  #[inline(always)]
  pub fn new(timeline: Timeline) -> Self {
    Self { timeline }
  }

  /// Start playing immediately (builder pattern).
  #[inline(always)]
  pub fn playing(mut self) -> Self {
    self.timeline.play();
    self
  }

  /// Play the timeline.
  #[inline(always)]
  pub fn play(&mut self) {
    self.timeline.play();
  }

  /// Pause the timeline.
  #[inline(always)]
  pub fn pause(&mut self) {
    self.timeline.pause();
  }

  /// Check if complete.
  #[inline(always)]
  pub fn is_complete(&self) -> bool {
    self.timeline.state().is_complete()
  }

  /// Get progress.
  #[inline(always)]
  pub fn progress(&self) -> f32 {
    self.timeline.progress()
  }

  /// Get the underlying timeline.
  #[inline(always)]
  pub fn timeline(&self) -> &Timeline {
    &self.timeline
  }

  /// Get mutable access to the underlying timeline.
  #[inline(always)]
  pub fn timeline_mut(&mut self) -> &mut Timeline {
    &mut self.timeline
  }
}

// ============================================================================
// Systems
// ============================================================================

/// System that ticks all animators of type T.
fn tick_animators<T: Tweenable>(
  mut query: Query<&mut Animator<T>>,
  time: Res<Time>,
) {
  let delta = time.delta_secs();

  for mut animator in &mut query {
    animator.tween.tick(delta);
  }
}

/// System that ticks all timeline animators.
fn tick_timeline_animators(
  mut query: Query<&mut TimelineAnimator>,
  time: Res<Time>,
) {
  let delta = time.delta_secs();

  for mut animator in &mut query {
    animator.timeline.tick(delta);
  }
}

// ============================================================================
// Convenience Constructors
// ============================================================================

/// Convenience functions for creating common tweens.
pub mod tweens {
  use super::*;

  /// Create a tween for f32 values.
  #[inline(always)]
  pub fn float(from: f32, to: f32) -> eazy_tweener::TweenBuilder<f32> {
    Tween::to(from, to)
  }

  /// Create a tween for Vec3 (requires implementing Tweenable for Vec3).
  #[inline(always)]
  pub fn vec3(
    from: [f32; 3],
    to: [f32; 3],
  ) -> eazy_tweener::TweenBuilder<[f32; 3]> {
    Tween::to(from, to)
  }

  /// Create a tween for Vec2-like values.
  #[inline(always)]
  pub fn vec2(
    from: [f32; 2],
    to: [f32; 2],
  ) -> eazy_tweener::TweenBuilder<[f32; 2]> {
    Tween::to(from, to)
  }

  /// Create a tween for RGBA colors.
  #[inline(always)]
  pub fn color(
    from: [f32; 4],
    to: [f32; 4],
  ) -> eazy_tweener::TweenBuilder<[f32; 4]> {
    Tween::to(from, to)
  }
}

// ============================================================================
// Events (optional, for when animations complete)
// ============================================================================

/// Event fired when an animator completes.
#[derive(Event)]
pub struct AnimationComplete<T: Tweenable> {
  pub entity: Entity,
  pub _marker: std::marker::PhantomData<T>,
}

/// Plugin that adds completion events for a type.
#[derive(Default)]
pub struct EazyEventsPlugin<T: Tweenable>(std::marker::PhantomData<T>);

impl<T: Tweenable> Plugin for EazyEventsPlugin<T> {
  fn build(&self, app: &mut App) {
    app.add_event::<AnimationComplete<T>>().add_systems(
      Update,
      emit_completion_events::<T>.after(tick_animators::<T>),
    );
  }
}

fn emit_completion_events<T: Tweenable>(
  query: Query<(Entity, &Animator<T>), Changed<Animator<T>>>,
  mut events: EventWriter<AnimationComplete<T>>,
) {
  for (entity, animator) in &query {
    if animator.is_complete() {
      events.send(AnimationComplete {
        entity,
        _marker: std::marker::PhantomData,
      });
    }
  }
}

// ============================================================================
// Re-exports for convenience
// ============================================================================

pub use eazy_core::{Curve, Easing};
pub use eazy_tweener::{
  Controllable, Direction, Position, Repeat, RepeatConfig, Stagger,
  StaggerFrom, Timeline, TimelineBuilder, Tween, TweenBuilder, TweenState,
  Tweenable,
};
