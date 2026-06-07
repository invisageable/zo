pub mod codec;
pub mod loader;
pub mod style;
pub mod ui;
mod ui_protocol;

pub use ui::Ui;
pub use ui_protocol::{
  Attr, ElementTag, EventKind, LIST_ITEM_SENTINEL, PropValue, StyleScope,
  UiCommand,
};

/// Whether a directive name mounts a template onto the active
/// render target. `render` is the sole name across every target
/// (egui, web DOM, UIKit, Android views) — "dom" was web-specific
/// and named only one of the four.
pub fn is_render_directive(name: &str) -> bool {
  matches!(name, "render")
}
