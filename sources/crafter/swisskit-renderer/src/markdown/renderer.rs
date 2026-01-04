//! Generic markdown renderer infrastructure.
//!
//! This module provides traits and types for rendering markdown
//! in a backend-agnostic way.

use crate::markdown::state::RenderState;

use pulldown_cmark::{Event, Tag, TagEnd};

/// Trait for implementing markdown rendering to different backends.
pub trait MarkdownRenderer {
  /// The UI context type (e.g., egui::Ui, Html builder, etc.).
  type Context;

  /// Handle a start tag.
  fn handle_start_tag(
    ctx: &mut Self::Context,
    state: &mut RenderState,
    tag: Tag,
  );

  /// Handle an end tag.
  fn handle_end_tag(
    ctx: &mut Self::Context,
    state: &mut RenderState,
    tag: TagEnd,
  );

  /// Handle text content.
  fn handle_text(ctx: &mut Self::Context, state: &mut RenderState, text: &str);

  /// Handle inline code.
  fn handle_inline_code(
    ctx: &mut Self::Context,
    state: &mut RenderState,
    code: &str,
  );

  /// Handle a soft break (space in normal text, newline in code).
  fn handle_soft_break(ctx: &mut Self::Context, state: &mut RenderState);

  /// Handle a hard break (explicit newline).
  fn handle_hard_break(ctx: &mut Self::Context, state: &mut RenderState);

  /// Handle a horizontal rule.
  fn handle_rule(ctx: &mut Self::Context, state: &mut RenderState);

  /// Handle an image.
  fn handle_image(
    ctx: &mut Self::Context,
    state: &mut RenderState,
    url: &str,
    title: &str,
    alt_text: &str,
  );

  /// Process a markdown event.
  fn process_event(
    ctx: &mut Self::Context,
    state: &mut RenderState,
    event: Event,
  ) {
    match event {
      Event::Start(tag) => Self::handle_start_tag(ctx, state, tag),
      Event::End(tag) => Self::handle_end_tag(ctx, state, tag),
      Event::Text(text) => Self::handle_text(ctx, state, &text),
      Event::Code(code) => Self::handle_inline_code(ctx, state, &code),
      Event::SoftBreak => Self::handle_soft_break(ctx, state),
      Event::HardBreak => Self::handle_hard_break(ctx, state),
      Event::Rule => Self::handle_rule(ctx, state),
      _ => {} // Handle other events as needed
    }
  }
}
