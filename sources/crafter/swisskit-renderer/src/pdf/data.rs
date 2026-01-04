/// Rendered page as RGBA bitmap.
pub struct RenderedPage {
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<u8>,
}

/// Character with position information for text selection.
#[derive(Debug, Clone)]
pub struct TextChar {
  /// The character.
  pub ch: char,
  /// Bounding box in page coordinates (points, 72 DPI).
  pub x: f32,
  pub y: f32,
  pub width: f32,
  pub height: f32,
}

/// Text content of a page with character positions.
#[derive(Debug, Clone, Default)]
pub struct PageText {
  /// All characters with positions.
  pub chars: Vec<TextChar>,
  /// Full text content.
  pub text: String,
}
