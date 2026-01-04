use pulldown_cmark::HeadingLevel;

use std::path::PathBuf;

/// State for tracking nested markdown elements during rendering.
#[derive(Debug, Default)]
pub struct RenderState {
  /// Current heading level for proper styling.
  pub current_heading_level: Option<HeadingLevel>,
  /// Whether we're inside a code block.
  pub in_code_block: bool,
  /// Code block language if specified.
  pub code_block_lang: Option<String>,
  /// Buffer for accumulating text in certain contexts.
  pub text_buffer: String,
  /// Whether we're in a paragraph.
  pub in_paragraph: bool,
  /// Whether we're in a blockquote.
  pub in_blockquote: bool,
  /// List depth for nested lists.
  pub list_depth: usize,
  /// Whether the current list is ordered.
  pub is_ordered_list: bool,
  /// Current ordered list counter.
  pub list_counter: usize,
  /// Whether we're inside an image tag.
  pub in_image: bool,
  /// Current image URL.
  pub image_url: Option<String>,
  /// Current image title.
  pub image_title: String,
  /// Current image alt text buffer.
  pub image_alt_text: String,
  /// Base path for resolving relative image URLs.
  pub base_path: Option<PathBuf>,
  /// Collected heading positions for scroll progress tracking.
  /// Each entry is (heading_text, y_position).
  pub headings: Vec<(String, f32)>,
}
