//! eazy-keyframes — Keyframe-based animation system.
//!
//! Provides keyframe tracks for defining complex animations with
//! values at specific time points and per-segment easing.
//!
//! # Examples
//!
//! ## Using the builder API
//!
//! ```rust
//! use eazy_keyframes::{KeyframeTrack, Keyframe};
//! use eazy_data::Easing;
//!
//! let track = KeyframeTrack::new()
//!   .keyframe(0.0, [0.0_f32, 0.0, 0.0])
//!   .keyframe_eased(0.5, [100.0, 50.0, 0.0], Easing::OutElastic)
//!   .keyframe(1.0, [0.0, 100.0, 0.0]);
//!
//! // Sample at 75% through the animation
//! let position = track.sample(0.75);
//! ```
//!
//! ## Using the keyframes! macro
//!
//! ```rust
//! use eazy_keyframes::{keyframes, Easing};
//!
//! let track = keyframes![
//!   (0.0, 0.0_f32),                      // (time, value) - linear
//!   (0.5, 100.0, Easing::OutBounce),     // (time, value, easing)
//!   (1.0, 50.0)
//! ];
//!
//! let value = track.sample(0.75);
//! ```

pub mod keyframe;
pub mod track;

// Re-export main types.
pub use keyframe::keyframe;
pub use keyframe::keyframe_eased;
pub use keyframe::Keyframe;
pub use track::KeyframeTrack;

// Re-export dependencies for convenience.
pub use eazy_data::Easing;
pub use eazy_tweener::Tweenable;

/// Create a [`KeyframeTrack`] from a list of keyframe tuples.
///
/// Supports two tuple formats:
/// - `(time, value)` — keyframe with linear easing
/// - `(time, value, easing)` — keyframe with custom easing
///
/// # Examples
///
/// ```rust
/// use eazy_keyframes::{keyframes, Easing};
///
/// // Simple f32 animation
/// let track = keyframes![
///   (0.0, 0.0_f32),
///   (0.5, 100.0, Easing::OutBounce),
///   (1.0, 50.0)
/// ];
///
/// // Array animation (e.g., position)
/// let track = keyframes![
///   (0.0, [0.0_f32, 0.0]),
///   (0.5, [100.0, 50.0], Easing::OutElastic),
///   (1.0, [0.0, 100.0])
/// ];
/// ```
#[macro_export]
macro_rules! keyframes {
  // Empty case.
  () => {
    $crate::KeyframeTrack::new()
  };

  // One or more keyframes.
  ($($kf:expr),+ $(,)?) => {{
    let keyframes: ::std::vec::Vec<$crate::Keyframe<_>> = ::std::vec![
      $($kf.into()),+
    ];

    $crate::KeyframeTrack::from(keyframes)
  }};
}
