use crate::pdf::data::{PageText, RenderedPage, TextChar};
use crate::pdf::error::PdfError;

pub use crate::pdf::platform;

use pdfium_render::prelude::PdfRenderConfig;

use std::path::Path;

/// PDF document for rendering.
/// Stores raw bytes and renders on-demand since PdfDocument has lifetime
/// constraints.
pub struct PdfRenderer {
  bytes: Vec<u8>,
  page_count: usize,
}

impl PdfRenderer {
  /// Parse PDF from bytes.
  pub fn from_bytes(bytes: &[u8]) -> Result<Self, PdfError> {
    let pdfium =
      platform::bind_pdfium().map_err(|e| PdfError::Init(format!("{e}")))?;

    let document = pdfium
      .load_pdf_from_byte_slice(bytes, None)
      .map_err(|e| PdfError::Parse(format!("{e}")))?;

    let page_count = document.pages().len() as usize;

    Ok(Self {
      bytes: bytes.to_vec(),
      page_count,
    })
  }

  /// Parse PDF from file.
  pub fn from_path(path: &Path) -> Result<Self, PdfError> {
    let bytes = std::fs::read(path)?;
    Self::from_bytes(&bytes)
  }

  /// Get page count.
  pub fn page_count(&self) -> usize {
    self.page_count
  }

  /// Render page at index to RGBA bitmap.
  /// Scale: 1.0 = 72 DPI, 2.0 = 144 DPI, etc.
  pub fn render_page(
    &self,
    index: usize,
    scale: f32,
  ) -> Result<RenderedPage, PdfError> {
    let pdfium =
      platform::bind_pdfium().map_err(|e| PdfError::Init(format!("{e}")))?;

    let document = pdfium
      .load_pdf_from_byte_slice(&self.bytes, None)
      .map_err(|e| PdfError::Parse(format!("{e}")))?;

    let page = document
      .pages()
      .get(index as u16)
      .map_err(|e| PdfError::Render(format!("Page {index} not found: {e}")))?;

    let width = (page.width().value * scale) as u32;
    let height = (page.height().value * scale) as u32;

    let config = PdfRenderConfig::new()
      .set_target_width(width as i32)
      .set_target_height(height as i32)
      .render_form_data(true)
      .render_annotations(true);

    let bitmap = page.render_with_config(&config).map_err(|e| {
      PdfError::Render(format!("Failed to render page {index}: {e}"))
    })?;

    let pixels = bitmap.as_rgba_bytes().to_vec();

    Ok(RenderedPage {
      width,
      height,
      pixels,
    })
  }

  /// Extract text content and character positions from a page.
  pub fn extract_text(&self, index: usize) -> Result<PageText, PdfError> {
    let pdfium =
      platform::bind_pdfium().map_err(|e| PdfError::Init(format!("{e}")))?;

    let document = pdfium
      .load_pdf_from_byte_slice(&self.bytes, None)
      .map_err(|e| PdfError::Parse(format!("{e}")))?;

    let page = document
      .pages()
      .get(index as u16)
      .map_err(|e| PdfError::Render(format!("Page {index} not found: {e}")))?;

    let page_height = page.height().value;
    let text_page = page.text().map_err(|e| {
      PdfError::Render(format!("Failed to get text from page {index}: {e}"))
    })?;

    let full_text = text_page.all();
    let mut chars = Vec::new();

    // Iterate over all characters in the page
    for char_info in text_page.chars().iter() {
      if let Some(ch) = char_info.unicode_char()
        && let Ok(bounds) = char_info.loose_bounds()
      {
        // Convert from PDF coordinates (origin bottom-left) to screen coords
        // (origin top-left)
        let x = bounds.left().value;
        let y = page_height - bounds.top().value;
        let width = bounds.right().value - bounds.left().value;
        let height = bounds.top().value - bounds.bottom().value;

        chars.push(TextChar {
          ch,
          x,
          y,
          width,
          height,
        });
      }
    }

    Ok(PageText {
      chars,
      text: full_text,
    })
  }
}
