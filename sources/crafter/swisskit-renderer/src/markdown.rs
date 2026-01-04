//! Markdown rendering infrastructure.
//!
//! This module provides utilities for parsing and rendering markdown,
//! with built-in XSS protection for HTML output.
//!
//! ## Usage
//!
//! ### For HTML output (recommended - includes sanitization):
//! ```rust
//! use swisskit_renderer::markdown::to_html;
//!
//! let markdown = "[link](http://example.com/)";
//! let safe_html = to_html(markdown);
//! ```
//!
//! ### For custom renderers (like egui):
//! ```rust
//! use swisskit_renderer::markdown::{create_parser, MarkdownRenderer};
//!
//! let parser = create_parser("# Hello");
//! // Use parser with your custom renderer
//! ```

pub mod parser;
mod renderer;
mod state;

pub use renderer::MarkdownRenderer;
pub use state::RenderState;
