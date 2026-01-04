//! WebView wrapper for cross-platform HTML rendering.
//!
//! Provides a generic WebView component using wry that can be embedded
//! in egui applications for rendering HTML content.

mod renderer;

pub use renderer::HtmlRenderer;
