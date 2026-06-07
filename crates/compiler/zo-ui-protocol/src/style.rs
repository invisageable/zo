//! Style — shared style data between native and web renderers.
//!
//! The cascade is the same model as the CSS cascade, restricted to
//! three layers (UA → author → inline). Pure data + pure
//! functions; no egui or DOM dependency. Native renderers consume
//! `ComputedStyle` directly; the web renderer serializes the same
//! UA table to a CSS string and lets the browser do the work.

pub mod cascade;
pub mod computed;
pub mod css;
pub mod ua;

pub use cascade::resolve;
pub use computed::{
  Align, ComputedStyle, Display, Edges, FlexDirection, FontFamily, FontStyle,
  GlassStyle, Justify, Material, Rgba, Size, StylePatch, TextAlign,
  TextDecoration,
};
pub use ua::{UA_SHEET, lookup as ua_lookup};
