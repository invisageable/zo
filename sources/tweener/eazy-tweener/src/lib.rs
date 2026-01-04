//! eazy-tweener — GSAP-like animation runtime for Rust.
//!
//! Built on top of eazy-data's mathematical foundation (96 easing curves,
//! interpolation functions, SIMD support), this crate provides a complete
//! animation runtime with tweens, timelines, callbacks, and stagger support.
//!
//! # Examples
//!
//! ## Simple Tween
//!
//! ```rust
//! use eazy_tweener::{Tween, Controllable};
//! use eazy_core::Easing;
//!
//! let mut tween = Tween::to(0.0_f32, 100.0)
//!   .duration(1.0)
//!   .easing(Easing::OutBounce)
//!   .on_complete(|| println!("Done!"))
//!   .build();
//!
//! tween.play();
//!
//! // In your update loop:
//! while tween.tick(0.016) {
//!   let value = tween.value();
//!   // Apply value to your target
//! }
//! ```
//!
//! ## Timeline with Sequencing
//!
//! ```rust
//! use eazy_tweener::{Timeline, Tween, Position, Controllable};
//!
//! let mut timeline = Timeline::builder()
//!   .push(Tween::to(0.0_f32, 100.0).duration(1.0).build())
//!   .push_at(
//!     Tween::to(0.0_f32, 50.0).duration(0.5).build(),
//!     Position::WithPrevious
//!   )
//!   .push_at(
//!     Tween::to(100.0_f32, 0.0).duration(1.0).build(),
//!     Position::Relative(-0.2)
//!   )
//!   .build();
//!
//! timeline.play();
//! ```
//!
//! ## Staggered Animations
//!
//! ```rust
//! use eazy_tweener::{Timeline, Tween, Stagger, StaggerFrom, Controllable};
//!
//! let tweens: Vec<_> = (0..5)
//!   .map(|_| Tween::to(0.0_f32, 100.0).duration(0.5).build())
//!   .collect();
//!
//! let mut timeline = Timeline::builder()
//!   .push_staggered(tweens, Stagger::each(0.1).from(StaggerFrom::Center))
//!   .build();
//!
//! timeline.play();
//! ```
//!
//! ## Custom Tweenable Types
//!
//! ```rust,ignore
//! use eazy_tweener::Tweenable;
//!
//! #[derive(Clone, Copy, Tweenable)]
//! struct Position {
//!   x: f32,
//!   y: f32,
//! }
//!
//! let a = Position { x: 0.0, y: 0.0 };
//! let b = Position { x: 100.0, y: 200.0 };
//! let mid = a.lerp(b, 0.5);  // Position { x: 50.0, y: 100.0 }
//! ```

pub mod callback;
pub mod control;
pub mod position;
pub mod repeat;
pub mod stagger;
pub mod timeline;
pub mod tween;
pub mod value;

// Re-export main types at crate root.
pub use callback::*;
pub use control::*;
pub use position::*;
pub use repeat::*;
pub use stagger::*;
pub use timeline::*;
pub use tween::*;
pub use value::*;
