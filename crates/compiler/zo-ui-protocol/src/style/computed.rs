//! Computed style — the resolved visual properties of a single
//! element after the cascade. Pure data, target-agnostic.
//!
//! Native renderers (egui) read this struct directly to drive
//! `RichText` + layout primitives. The web renderer converts the
//! UA sheet to a CSS string once at startup and lets the browser
//! do the work — it does not read `ComputedStyle` at runtime.

use serde::{Deserialize, Serialize};

/// CSS box layout mode. Phase 1 only honors `Block` and `Inline`;
/// `Flex` and `Grid` are reserved for the Taffy integration in
/// Phase 3.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum Display {
  #[default]
  Block,
  Inline,
  Flex,
  Grid,
  None,
}

/// Generic font family bucket. Mapped to a concrete font by the
/// renderer (egui's proportional/monospace; CSS's
/// sans-serif/serif/monospace on the web).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum FontFamily {
  #[default]
  Sans,
  Serif,
  Mono,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum FontStyle {
  #[default]
  Normal,
  Italic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum TextAlign {
  #[default]
  Left,
  Center,
  Right,
  Justify,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum TextDecoration {
  #[default]
  None,
  Underline,
}

/// Length value with the small CSS unit set we care about. `Auto`
/// is the unspecified default; `Px` is absolute; `Percent` is a
/// fraction of the containing block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum Size {
  #[default]
  Auto,
  Px(f32),
  Percent(f32),
}

/// Four-sided length, in CSS order (top, right, bottom, left).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Edges {
  pub top: f32,
  pub right: f32,
  pub bottom: f32,
  pub left: f32,
}

impl Edges {
  pub const ZERO: Self = Self {
    top: 0.0,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  };

  /// Uniform edges (`margin: 8px` shorthand).
  pub const fn all(v: f32) -> Self {
    Self {
      top: v,
      right: v,
      bottom: v,
      left: v,
    }
  }

  /// Vertical-only edges (`margin: 16px 0` shorthand).
  pub const fn v(top: f32, bottom: f32) -> Self {
    Self {
      top,
      right: 0.0,
      bottom,
      left: 0.0,
    }
  }
}

/// Premultiplied 8-bit RGBA. Matches the byte layout used by both
/// the egui side (`Color32::from_rgba_unmultiplied`) and CSS
/// `rgba()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Rgba {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}

impl Rgba {
  pub const BLACK: Self = Self::rgb(0, 0, 0);
  pub const WHITE: Self = Self::rgb(255, 255, 255);
  /// HTML link blue (matches the WHATWG default for `:link`).
  pub const LINK_BLUE: Self = Self::rgb(0, 0, 238);
  pub const TRANSPARENT: Self = Self {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
  };

  pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
    Self { r, g, b, a: 255 }
  }
}

/// Fully resolved style for one element. Renderers consume this
/// after the cascade has folded UA + author + inline patches.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct ComputedStyle {
  // box model.
  pub display: Display,
  pub margin: Edges,
  pub padding: Edges,
  pub width: Size,
  pub height: Size,
  pub min_width: Size,
  pub min_height: Size,

  // typography.
  pub font_family: FontFamily,
  pub font_size: f32,
  pub font_weight: u16,
  pub font_style: FontStyle,
  pub text_align: TextAlign,
  pub text_decoration: TextDecoration,
  pub line_height: f32,

  // color.
  pub color: Rgba,
  pub background: Rgba,
}

impl ComputedStyle {
  /// Root defaults. Equivalent to a browser's initial values for
  /// the `html` element: white bg, black text, 16px sans, 400.
  pub const ROOT: Self = Self {
    display: Display::Block,
    margin: Edges::ZERO,
    padding: Edges::ZERO,
    width: Size::Auto,
    height: Size::Auto,
    min_width: Size::Auto,
    min_height: Size::Auto,
    font_family: FontFamily::Sans,
    font_size: 16.0,
    font_weight: 400,
    font_style: FontStyle::Normal,
    text_align: TextAlign::Left,
    text_decoration: TextDecoration::None,
    line_height: 1.2,
    color: Rgba::BLACK,
    background: Rgba::WHITE,
  };
}

impl Default for ComputedStyle {
  fn default() -> Self {
    Self::ROOT
  }
}

/// Sparse override applied during the cascade. Each `Some` field
/// overwrites the corresponding `ComputedStyle` field; `None`
/// leaves it untouched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct StylePatch {
  pub display: Option<Display>,
  pub margin: Option<Edges>,
  pub padding: Option<Edges>,
  pub width: Option<Size>,
  pub height: Option<Size>,
  pub min_width: Option<Size>,
  pub min_height: Option<Size>,

  pub font_family: Option<FontFamily>,
  pub font_size: Option<f32>,
  pub font_weight: Option<u16>,
  pub font_style: Option<FontStyle>,
  pub text_align: Option<TextAlign>,
  pub text_decoration: Option<TextDecoration>,
  pub line_height: Option<f32>,

  pub color: Option<Rgba>,
  pub background: Option<Rgba>,
}

impl StylePatch {
  pub const EMPTY: Self = Self {
    display: None,
    margin: None,
    padding: None,
    width: None,
    height: None,
    min_width: None,
    min_height: None,
    font_family: None,
    font_size: None,
    font_weight: None,
    font_style: None,
    text_align: None,
    text_decoration: None,
    line_height: None,
    color: None,
    background: None,
  };

  /// Fold this patch into a `ComputedStyle`. Set fields overwrite;
  /// unset fields leave the base untouched.
  pub fn apply(&self, base: &mut ComputedStyle) {
    if let Some(v) = self.display {
      base.display = v;
    }
    if let Some(v) = self.margin {
      base.margin = v;
    }
    if let Some(v) = self.padding {
      base.padding = v;
    }
    if let Some(v) = self.width {
      base.width = v;
    }
    if let Some(v) = self.height {
      base.height = v;
    }
    if let Some(v) = self.min_width {
      base.min_width = v;
    }
    if let Some(v) = self.min_height {
      base.min_height = v;
    }
    if let Some(v) = self.font_family {
      base.font_family = v;
    }
    if let Some(v) = self.font_size {
      base.font_size = v;
    }
    if let Some(v) = self.font_weight {
      base.font_weight = v;
    }
    if let Some(v) = self.font_style {
      base.font_style = v;
    }
    if let Some(v) = self.text_align {
      base.text_align = v;
    }
    if let Some(v) = self.text_decoration {
      base.text_decoration = v;
    }
    if let Some(v) = self.line_height {
      base.line_height = v;
    }
    if let Some(v) = self.color {
      base.color = v;
    }
    if let Some(v) = self.background {
      base.background = v;
    }
  }
}
