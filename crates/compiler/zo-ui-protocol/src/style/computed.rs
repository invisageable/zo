//! Computed style — the resolved visual properties of a single
//! element after the cascade. Pure data, target-agnostic.
//!
//! Native renderers (egui) read this struct directly to drive
//! `RichText` + layout primitives. The web renderer converts the
//! UA sheet to a CSS string once at startup and lets the browser
//! do the work — it does not read `ComputedStyle` at runtime.

use serde::{Deserialize, Serialize};

/// CSS box layout mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum Display {
  #[default]
  Block,
  Inline,
  Flex,
  Grid,
  None,
}

/// Flex main axis. `Row` lays children left-to-right, `Column`
/// top-to-bottom. The only two directions zo's layout needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum FlexDirection {
  #[default]
  Row,
  Column,
}

/// Main-axis distribution of flex children (`justify-content`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum Justify {
  #[default]
  Start,
  Center,
  End,
  SpaceBetween,
}

/// Cross-axis alignment of flex children (`align-items`). `Stretch`
/// is the CSS default; the others pin children to one edge or centre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum Align {
  #[default]
  Stretch,
  Start,
  Center,
  End,
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

/// The surface material an element paints with. `Glass` opts into
/// Apple's Liquid Glass on iOS 26 (a frosted approximation on web /
/// egui); a declared `background` colour tints the glass instead of
/// filling it solid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum Material {
  /// A solid fill — today's behaviour, byte-identical default.
  #[default]
  Solid,
  /// A translucent glass material in the given style.
  Glass(GlassStyle),
}

/// The Liquid Glass variant, mirroring `UIGlassEffectStyle`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum GlassStyle {
  /// Standard glass — the more opaque, legible default.
  #[default]
  Regular,
  /// Clear glass — thinner, lets more of the backdrop through.
  Clear,
}

/// Flex line wrapping.
#[derive(
  Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize,
)]
pub enum FlexWrap {
  #[default]
  NoWrap,
  Wrap,
}

/// A drop shadow behind the element's box. Kept POD so
/// `ComputedStyle` stays `Copy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Shadow {
  pub offset_x: f32,
  pub offset_y: f32,
  pub blur: f32,
  pub color: Rgba,
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
  pub max_width: Size,
  pub max_height: Size,
  /// Width / height; `0.0` means unconstrained (no aspect lock).
  /// An f32 sentinel rather than `Option` keeps the struct POD.
  pub aspect_ratio: f32,

  // flex.
  pub flex_direction: FlexDirection,
  pub justify_content: Justify,
  pub align_items: Align,
  pub gap: f32,
  pub flex_wrap: FlexWrap,
  pub flex_grow: f32,
  pub flex_shrink: f32,

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

  // surface.
  pub material: Material,
  /// An image painted behind the element, as an index into the
  /// stylesheet's image catalog (`css::ParsedSheet::images`). `None`
  /// means no image. A `u32` handle keeps this struct POD + `Copy`.
  pub background_image: Option<u32>,

  // border + shadow.
  /// Uniform border width; `0.0` paints no border.
  pub border_width: f32,
  pub border_color: Rgba,
  /// Uniform corner radius in pixels.
  pub border_radius: f32,
  /// `None` paints no shadow.
  pub box_shadow: Option<Shadow>,
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
    max_width: Size::Auto,
    max_height: Size::Auto,
    aspect_ratio: 0.0,
    flex_direction: FlexDirection::Row,
    justify_content: Justify::Start,
    align_items: Align::Stretch,
    gap: 0.0,
    flex_wrap: FlexWrap::NoWrap,
    flex_grow: 0.0,
    flex_shrink: 1.0,
    font_family: FontFamily::Sans,
    font_size: 16.0,
    font_weight: 400,
    font_style: FontStyle::Normal,
    text_align: TextAlign::Left,
    text_decoration: TextDecoration::None,
    line_height: 1.2,
    color: Rgba::BLACK,
    background: Rgba::WHITE,
    material: Material::Solid,
    background_image: None,
    border_width: 0.0,
    border_color: Rgba::BLACK,
    border_radius: 0.0,
    box_shadow: None,
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
  pub max_width: Option<Size>,
  pub max_height: Option<Size>,
  pub aspect_ratio: Option<f32>,

  pub flex_direction: Option<FlexDirection>,
  pub justify_content: Option<Justify>,
  pub align_items: Option<Align>,
  pub gap: Option<f32>,
  pub flex_wrap: Option<FlexWrap>,
  pub flex_grow: Option<f32>,
  pub flex_shrink: Option<f32>,

  pub font_family: Option<FontFamily>,
  pub font_size: Option<f32>,
  pub font_weight: Option<u16>,
  pub font_style: Option<FontStyle>,
  pub text_align: Option<TextAlign>,
  pub text_decoration: Option<TextDecoration>,
  pub line_height: Option<f32>,

  pub color: Option<Rgba>,
  pub background: Option<Rgba>,

  pub material: Option<Material>,
  pub background_image: Option<u32>,

  pub border_width: Option<f32>,
  pub border_color: Option<Rgba>,
  pub border_radius: Option<f32>,
  pub box_shadow: Option<Shadow>,
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
    max_width: None,
    max_height: None,
    aspect_ratio: None,
    flex_direction: None,
    justify_content: None,
    align_items: None,
    gap: None,
    flex_wrap: None,
    flex_grow: None,
    flex_shrink: None,
    font_family: None,
    font_size: None,
    font_weight: None,
    font_style: None,
    text_align: None,
    text_decoration: None,
    line_height: None,
    color: None,
    background: None,
    material: None,
    background_image: None,
    border_width: None,
    border_color: None,
    border_radius: None,
    box_shadow: None,
  };

  /// Merge another patch on top of this one. Set fields in `other`
  /// overwrite; unset fields leave this patch's field untouched. Used
  /// to fold several author rules that target the same tag into one.
  pub fn overlay(&mut self, other: &Self) {
    if other.display.is_some() {
      self.display = other.display;
    }
    if other.margin.is_some() {
      self.margin = other.margin;
    }
    if other.padding.is_some() {
      self.padding = other.padding;
    }
    if other.width.is_some() {
      self.width = other.width;
    }
    if other.height.is_some() {
      self.height = other.height;
    }
    if other.min_width.is_some() {
      self.min_width = other.min_width;
    }
    if other.min_height.is_some() {
      self.min_height = other.min_height;
    }
    if other.flex_direction.is_some() {
      self.flex_direction = other.flex_direction;
    }
    if other.justify_content.is_some() {
      self.justify_content = other.justify_content;
    }
    if other.align_items.is_some() {
      self.align_items = other.align_items;
    }
    if other.gap.is_some() {
      self.gap = other.gap;
    }
    if other.font_family.is_some() {
      self.font_family = other.font_family;
    }
    if other.font_size.is_some() {
      self.font_size = other.font_size;
    }
    if other.font_weight.is_some() {
      self.font_weight = other.font_weight;
    }
    if other.font_style.is_some() {
      self.font_style = other.font_style;
    }
    if other.text_align.is_some() {
      self.text_align = other.text_align;
    }
    if other.text_decoration.is_some() {
      self.text_decoration = other.text_decoration;
    }
    if other.line_height.is_some() {
      self.line_height = other.line_height;
    }
    if other.color.is_some() {
      self.color = other.color;
    }
    if other.background.is_some() {
      self.background = other.background;
    }
    if other.material.is_some() {
      self.material = other.material;
    }
    if other.background_image.is_some() {
      self.background_image = other.background_image;
    }
    if other.max_width.is_some() {
      self.max_width = other.max_width;
    }
    if other.max_height.is_some() {
      self.max_height = other.max_height;
    }
    if other.aspect_ratio.is_some() {
      self.aspect_ratio = other.aspect_ratio;
    }
    if other.flex_wrap.is_some() {
      self.flex_wrap = other.flex_wrap;
    }
    if other.flex_grow.is_some() {
      self.flex_grow = other.flex_grow;
    }
    if other.flex_shrink.is_some() {
      self.flex_shrink = other.flex_shrink;
    }
    if other.border_width.is_some() {
      self.border_width = other.border_width;
    }
    if other.border_color.is_some() {
      self.border_color = other.border_color;
    }
    if other.border_radius.is_some() {
      self.border_radius = other.border_radius;
    }
    if other.box_shadow.is_some() {
      self.box_shadow = other.box_shadow;
    }
  }

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
    if let Some(v) = self.flex_direction {
      base.flex_direction = v;
    }
    if let Some(v) = self.justify_content {
      base.justify_content = v;
    }
    if let Some(v) = self.align_items {
      base.align_items = v;
    }
    if let Some(v) = self.gap {
      base.gap = v;
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
    if let Some(v) = self.material {
      base.material = v;
    }
    // Both sides are `Option<u32>` here (the resolved style also holds
    // a handle, not a value), so set it whole rather than unwrap.
    if self.background_image.is_some() {
      base.background_image = self.background_image;
    }
    if let Some(v) = self.max_width {
      base.max_width = v;
    }
    if let Some(v) = self.max_height {
      base.max_height = v;
    }
    if let Some(v) = self.aspect_ratio {
      base.aspect_ratio = v;
    }
    if let Some(v) = self.flex_wrap {
      base.flex_wrap = v;
    }
    if let Some(v) = self.flex_grow {
      base.flex_grow = v;
    }
    if let Some(v) = self.flex_shrink {
      base.flex_shrink = v;
    }
    if let Some(v) = self.border_width {
      base.border_width = v;
    }
    if let Some(v) = self.border_color {
      base.border_color = v;
    }
    if let Some(v) = self.border_radius {
      base.border_radius = v;
    }
    // `Option` on both sides — set whole, like background_image.
    if self.box_shadow.is_some() {
      base.box_shadow = self.box_shadow;
    }
  }
}
