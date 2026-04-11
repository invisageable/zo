//! Egui-based renderer for UI commands

use crate::loader::image::{ImageLoader, ImageState};

use zo_runtime_render::render::{EventId, Render, WidgetId};
use zo_ui_protocol::style::{ComputedStyle, FontFamily, Rgba, cascade};
use zo_ui_protocol::{Attr, ElementTag, UiCommand};

use eframe::egui;
use rustc_hash::FxHashMap as HashMap;
use thin_vec::ThinVec;

/// Parsed style properties for a CSS rule.
#[derive(Clone, Default)]
struct StyleProps {
  color: Option<egui::Color32>,
  bold: bool,
}

/// State for managing UI elements
#[derive(Default)]
pub struct UiState {
  /// Text input values indexed by ID
  text_inputs: HashMap<u32, String>,
  /// Button click events to send back
  pending_events: ThinVec<(u32, u32)>, // (widget_id, event_kind)
}

/// Egui-based renderer for zo UI commands
pub struct Renderer {
  state: UiState,
  /// Store commands for rendering in the egui context
  pending_commands: ThinVec<UiCommand>,
  /// Selector → style props, built from StyleSheet commands.
  styles: HashMap<String, StyleProps>,
  /// Async image loader — decodes on a worker thread,
  /// uploads textures to the GPU on the main thread.
  image_loader: ImageLoader,
}

impl Renderer {
  /// Creates native [`Renderer`] instance.
  pub fn new() -> Self {
    Self {
      state: UiState::default(),
      pending_commands: ThinVec::new(),
      styles: HashMap::default(),
      image_loader: ImageLoader::new(),
    }
  }

  /// Render commands with an egui UI context
  pub fn render_with_ui(&mut self, ui: &mut egui::Ui) {
    // Drain pending image loads before rendering.
    self.image_loader.poll();

    if !self.pending_commands.is_empty() {
      let commands = std::mem::take(&mut self.pending_commands);

      // Pre-scan: parse StyleSheet commands into the
      // styles map before rendering elements.
      self.styles.clear();

      for cmd in &commands {
        if let UiCommand::StyleSheet { css, .. } = cmd {
          parse_css_into(&mut self.styles, css);
        }
      }

      // Top-level inline-flow detection. Templates that use a
      // fragment (`<></>`) emit their children as siblings at the
      // root, so the inline-flow check inside `render_element`
      // never fires for them. Mirror that branch here so a
      // counter `<><button/>{n}<button/></>` lays out
      // horizontally, like its block-parent equivalent would.
      if children_are_inline_flow(&commands, 0) {
        ui.horizontal(|ui| self.render_commands(ui, &commands, 0));
      } else {
        self.render_commands(ui, &commands, 0);
      }
    }
  }

  /// Recursively render commands
  fn render_commands(
    &mut self,
    ui: &mut egui::Ui,
    commands: &[UiCommand],
    start_idx: usize,
  ) -> usize {
    let mut idx = start_idx;

    while idx < commands.len() {
      match &commands[idx] {
        UiCommand::Event { .. } => {
          // events are handled separately.
          idx += 1;
        }

        UiCommand::StyleSheet { .. } => {
          // Native style mapping is post-MVP.
          idx += 1;
        }

        UiCommand::Element {
          tag,
          attrs,
          self_closing,
        } => {
          idx =
            self.render_element(ui, commands, idx, tag, attrs, *self_closing);
        }

        UiCommand::EndElement => {
          // Close current element — hand control back to the
          // caller (the enclosing container's closure).
          return idx + 1;
        }

        UiCommand::Text(content) => {
          ui.label(content);

          idx += 1;
        }
      }
    }

    idx
  }

  /// Render a single `UiCommand::Element` and any inline children
  /// up to the matching `EndElement`. Returns the index just after
  /// the element's close (or just after the element itself, if
  /// self-closing).
  fn render_element(
    &mut self,
    ui: &mut egui::Ui,
    commands: &[UiCommand],
    idx: usize,
    tag: &ElementTag,
    attrs: &[Attr],
    self_closing: bool,
  ) -> usize {
    let children_start = idx + 1;

    match tag {
      ElementTag::Img => {
        let src = attr_str(attrs, "src").unwrap_or("");
        let width = attr_num(attrs, "width").unwrap_or(0);
        let height = attr_num(attrs, "height").unwrap_or(0);

        self.render_image(ui, src, width, height);

        // Img is always self-closing — no children to skip.
        children_start
      }

      ElementTag::Input | ElementTag::Textarea => {
        let id = attr_num(attrs, "data-id").unwrap_or(0);
        let placeholder = attr_str(attrs, "placeholder").unwrap_or("");
        let initial = attr_str(attrs, "value").unwrap_or("").to_string();

        let text = self.state.text_inputs.entry(id).or_insert_with(|| initial);

        let response =
          ui.add(egui::TextEdit::singleline(text).hint_text(placeholder));

        if response.changed() {
          self.state.pending_events.push((id, 1));
        }

        // Self-closing in our model (input has no children).
        children_start
      }

      ElementTag::Button => {
        // Button label is the concatenation of its TextNode
        // children. Render the button imperatively, then skip
        // past children to the matching EndElement.
        //
        // No cascade lookup yet — `UA_SHEET` has no `button`
        // entry, so the resolved style would just overwrite
        // egui's tuned button defaults (font, padding) with
        // `ROOT` values. When PLAN_STYLER lands and we add a
        // real `button` UA entry, this branch flows through
        // `cascade::resolve("button", author, inline)` and
        // applies font + min_size on the `egui::Button`.
        let content = peek_text_children(commands, children_start);
        let id = attr_num(attrs, "data-id").unwrap_or(0);

        if ui.button(&content).clicked() {
          self.state.pending_events.push((id, 0));
        }

        skip_to_end_element(commands, children_start)
      }

      t if t.is_text_tag() => {
        // h1/h2/h3/p/span with inline PCDATA — render as a styled
        // label. Concatenate all TextNode children and skip past
        // them.
        let content = peek_text_children(commands, children_start);

        self.render_styled_text_for_tag(ui, &content, tag);

        if self_closing {
          children_start
        } else {
          skip_to_end_element(commands, children_start)
        }
      }

      t if t.is_inline() => {
        // Inline container with non-text children → horizontal.
        if self_closing {
          children_start
        } else {
          ui.horizontal(|ui| self.render_commands(ui, commands, children_start))
            .inner
        }
      }

      _ => {
        // Block container (div, section, main, ...).
        //
        // Pre-Taffy hack for inline flow: if every direct child is
        // an inline-ish element (span, button, input) or raw text,
        // lay them out horizontally so siblings flow on a single
        // line — matching how the web treats `inline-block`
        // children inside a block parent. Mixed/all-block children
        // fall back to vertical block flow. Phase 3 (Taffy)
        // replaces this with the real CSS algorithm.
        if self_closing {
          children_start
        } else if children_are_inline_flow(commands, children_start) {
          ui.horizontal(|ui| self.render_commands(ui, commands, children_start))
            .inner
        } else {
          ui.vertical(|ui| self.render_commands(ui, commands, children_start))
            .inner
        }
      }
    }
  }

  /// Render an image from an attribute-described element.
  fn render_image(
    &mut self,
    ui: &mut egui::Ui,
    src: &str,
    width: u32,
    height: u32,
  ) {
    let size = egui::Vec2::new(width as f32, height as f32);
    let ctx = ui.ctx().clone();
    let state = self.image_loader.state(src);

    match state {
      ImageState::Pending | ImageState::Loading => {
        ui.add_sized(size, egui::Spinner::new());
      }
      ImageState::Decoded(_) => {
        let image = match std::mem::replace(state, ImageState::Loading) {
          ImageState::Decoded(img) => img,
          _ => unreachable!(),
        };

        let texture =
          ctx.load_texture(src, image, egui::TextureOptions::default());

        ui.add(egui::Image::from_texture(&texture).fit_to_exact_size(size));

        *state = ImageState::Ready(texture);
      }
      ImageState::Ready(texture) => {
        ui.add(egui::Image::from_texture(&*texture).fit_to_exact_size(size));
      }
      ImageState::Failed(error) => {
        ui.colored_label(egui::Color32::RED, format!("[image error: {error}]"));
      }
    }
  }

  /// Render styled text for an `ElementTag` text tag. Resolves
  /// the computed style via the UA cascade so unstyled tags get
  /// browser-like defaults (h1=32px/700, p=16px/400, code=mono,
  /// ...). The author `styles` map still overrides on top until
  /// PLAN_STYLER feeds the cascade directly.
  fn render_styled_text_for_tag(
    &self,
    ui: &mut egui::Ui,
    content: &str,
    tag: &ElementTag,
  ) {
    let computed = cascade::resolve(tag.as_str(), None, None);
    let mut rt = apply_computed(egui::RichText::new(content), &computed);

    // Author overlay (legacy path — replaced in Phase 2 by passing
    // a real `StylePatch` into `cascade::resolve`).
    if let Some(props) = self.styles.get(tag.as_str()) {
      if let Some(c) = props.color {
        rt = rt.color(c);
      }

      if props.bold {
        rt = rt.strong();
      }
    }

    // Block-flow vertical margin: top before, bottom after. The
    // horizontal half (`margin.left/right`) is ignored at v1 — no
    // real box model yet, that lands with Taffy in Phase 3. Note:
    // browsers collapse adjacent vertical margins; egui's
    // `add_space` doesn't, so consecutive paragraphs will have
    // double the gap until margin collapsing lands in Phase 2.
    ui.add_space(computed.margin.top);
    ui.label(rt);
    ui.add_space(computed.margin.bottom);
  }

  /// Get pending events to send back to the application
  pub fn take_pending_events(&mut self) -> ThinVec<(u32, u32)> {
    std::mem::take(&mut self.state.pending_events)
  }
}

impl Render for Renderer {
  /// Queue commands for rendering
  fn render(&mut self, commands: &[UiCommand]) {
    self.pending_commands = commands.into();
  }

  /// Handle events from the UI
  fn handle_event(
    &mut self,
    widget_id: &WidgetId,
    event_id: &EventId,
    _event_data: ThinVec<u8>,
  ) {
    // This would be called by the application to handle events
    println!(
      "Renderer handling event: widget {} event {}",
      widget_id.0, event_id.0
    );
  }

  /// Initialize the renderer
  fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  /// Cleanup resources
  fn cleanup(&mut self) {
    self.pending_commands.clear();
    self.state.text_inputs.clear();
    self.state.pending_events.clear();
  }
}

impl Default for Renderer {
  fn default() -> Self {
    Self::new()
  }
}

// --- Element dispatch helpers ---

/// Look up the string value of a named attribute.
fn attr_str<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a str> {
  for attr in attrs {
    if attr.name() == name {
      return attr.as_str();
    }
  }

  None
}

/// Look up the numeric value of a named attribute.
fn attr_num(attrs: &[Attr], name: &str) -> Option<u32> {
  for attr in attrs {
    if attr.name() == name {
      return attr
        .as_num()
        .or_else(|| attr.as_str().and_then(|s| s.parse().ok()));
    }
  }

  None
}

/// Concatenate all `TextNode` children starting at `start`, up to
/// (but not including) the matching `EndElement`. Nested elements
/// are ignored — only direct text children contribute.
fn peek_text_children(commands: &[UiCommand], start: usize) -> String {
  let mut out = String::new();
  let mut depth: usize = 0;
  let mut idx = start;

  while idx < commands.len() {
    match &commands[idx] {
      UiCommand::Element { self_closing, .. } if !self_closing => {
        depth += 1;
      }
      UiCommand::EndElement => {
        if depth == 0 {
          break;
        }

        depth -= 1;
      }
      UiCommand::Text(s) if depth == 0 => {
        out.push_str(s);
      }
      _ => {}
    }

    idx += 1;
  }

  out
}

/// Return `true` when every direct child between `start` and the
/// matching `EndElement` is inline-flow (span, button, input,
/// textarea, img) or non-blank raw text. Empty containers return
/// `false`. Used by the block container branch to pick a
/// horizontal layout when its content would flow on a single line
/// on the web. Phase 3 (Taffy) replaces this with the real CSS
/// inline-formatting-context algorithm.
fn children_are_inline_flow(commands: &[UiCommand], start: usize) -> bool {
  let mut depth: usize = 0;
  let mut idx = start;
  let mut saw_any = false;

  while idx < commands.len() {
    match &commands[idx] {
      UiCommand::Element {
        tag, self_closing, ..
      } => {
        if depth == 0 {
          saw_any = true;

          if !is_inline_flow_tag(tag) {
            return false;
          }
        }

        if !self_closing {
          depth += 1;
        }
      }
      UiCommand::EndElement => {
        if depth == 0 {
          return saw_any;
        }

        depth -= 1;
      }
      UiCommand::Text(s) if depth == 0 => {
        if !s.trim().is_empty() {
          saw_any = true;
        }
      }
      _ => {}
    }

    idx += 1;
  }

  saw_any
}

/// Tags treated as inline flow at the parent-layout level. Mirrors
/// CSS `display: inline | inline-block` for the small subset zo
/// currently models.
fn is_inline_flow_tag(tag: &ElementTag) -> bool {
  matches!(
    tag,
    ElementTag::Span
      | ElementTag::Button
      | ElementTag::Input
      | ElementTag::Textarea
      | ElementTag::Img
  )
}

/// Return the index just after the matching `EndElement` for the
/// element whose children begin at `start`. If no matching
/// `EndElement` is found, return `commands.len()`.
fn skip_to_end_element(commands: &[UiCommand], start: usize) -> usize {
  let mut depth: usize = 0;
  let mut idx = start;

  while idx < commands.len() {
    match &commands[idx] {
      UiCommand::Element { self_closing, .. } if !self_closing => {
        depth += 1;
      }
      UiCommand::EndElement => {
        if depth == 0 {
          return idx + 1;
        }

        depth -= 1;
      }
      _ => {}
    }

    idx += 1;
  }

  commands.len()
}

// --- CSS → egui mapping ---

/// Parses a compiled CSS string and populates the styles map.
///
/// Handles the minimal subset: `selector { prop: val; }`.
/// The CSS is already compiled by zo-styler — selectors are
/// clean, properties are full names, values are raw strings.
fn parse_css_into(styles: &mut HashMap<String, StyleProps>, css: &str) {
  let mut chars = css.chars().peekable();

  while chars.peek().is_some() {
    // Skip whitespace.
    skip_ws(&mut chars);

    // Read selector (everything until `{`).
    let mut selector = String::new();

    while let Some(&ch) = chars.peek() {
      if ch == '{' {
        break;
      }

      selector.push(ch);
      chars.next();
    }

    let selector = selector.trim().to_string();

    if selector.is_empty() {
      break;
    }

    // Skip `{`.
    chars.next();

    // Read declarations until `}`.
    let mut props = StyleProps::default();

    loop {
      skip_ws(&mut chars);

      match chars.peek() {
        Some(&'}') => {
          chars.next();
          break;
        }
        None => break,
        _ => {}
      }

      // Property name (until `:`).
      let mut name = String::new();

      while let Some(&ch) = chars.peek() {
        if ch == ':' {
          break;
        }

        name.push(ch);
        chars.next();
      }

      // Skip `:`.
      chars.next();
      skip_ws(&mut chars);

      // Value (until `;` or `}`).
      let mut value = String::new();

      while let Some(&ch) = chars.peek() {
        if ch == ';' || ch == '}' {
          break;
        }

        value.push(ch);
        chars.next();
      }

      // Skip `;` if present.
      if chars.peek() == Some(&';') {
        chars.next();
      }

      let name = name.trim();
      let value = value.trim();

      match name {
        "color" => {
          props.color = parse_css_color(value);
        }
        "font-weight" => {
          // >= 700 is bold (CSS spec).
          if let Ok(w) = value.parse::<u32>() {
            props.bold = w >= 700;
          } else {
            props.bold = value == "bold";
          }
        }
        _ => {}
      }
    }

    // Strip scope hash from selector for lookup.
    // e.g. `p._zo_a3f2` → `p`
    let key = selector
      .split('.')
      .next()
      .unwrap_or(&selector)
      .trim()
      .to_string();

    styles.insert(key, props);
  }
}

fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars>) {
  while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
    chars.next();
  }
}

/// Parses a CSS color value to egui Color32.
///
/// Supports:
/// - Named colors: cyan, red, green, blue, gray, white, black, ...
/// - 3-digit hex: #f00
/// - 6-digit hex: #b2f5ea
fn parse_css_color(value: &str) -> Option<egui::Color32> {
  if let Some(hex) = value.strip_prefix('#') {
    return parse_hex_color(hex);
  }

  // Named CSS colors (common subset).
  match value {
    "black" => Some(egui::Color32::BLACK),
    "white" => Some(egui::Color32::WHITE),
    "red" => Some(egui::Color32::from_rgb(255, 0, 0)),
    "green" => Some(egui::Color32::from_rgb(0, 128, 0)),
    "blue" => Some(egui::Color32::from_rgb(0, 0, 255)),
    "cyan" => Some(egui::Color32::from_rgb(0, 255, 255)),
    "magenta" => Some(egui::Color32::from_rgb(255, 0, 255)),
    "yellow" => Some(egui::Color32::from_rgb(255, 255, 0)),
    "orange" => Some(egui::Color32::from_rgb(255, 165, 0)),
    "gray" | "grey" => Some(egui::Color32::GRAY),
    "transparent" => Some(egui::Color32::TRANSPARENT),
    _ => None,
  }
}

fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
  let bytes = hex.as_bytes();

  match bytes.len() {
    3 => {
      let r = hex_digit(bytes[0])? * 17;
      let g = hex_digit(bytes[1])? * 17;
      let b = hex_digit(bytes[2])? * 17;

      Some(egui::Color32::from_rgb(r, g, b))
    }
    6 => {
      let r = hex_byte(bytes[0], bytes[1])?;
      let g = hex_byte(bytes[2], bytes[3])?;
      let b = hex_byte(bytes[4], bytes[5])?;

      Some(egui::Color32::from_rgb(r, g, b))
    }
    _ => None,
  }
}

fn hex_digit(b: u8) -> Option<u8> {
  match b {
    b'0'..=b'9' => Some(b - b'0'),
    b'a'..=b'f' => Some(b - b'a' + 10),
    b'A'..=b'F' => Some(b - b'A' + 10),
    _ => None,
  }
}

fn hex_byte(hi: u8, lo: u8) -> Option<u8> {
  Some(hex_digit(hi)? * 16 + hex_digit(lo)?)
}

/// Convert a target-agnostic `Rgba` from `zo-ui-protocol::style`
/// into the egui `Color32` the renderer talks to.
fn rgba_to_color32(c: Rgba) -> egui::Color32 {
  egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Apply a `ComputedStyle` to an `egui::RichText`. egui's text
/// API is intentionally narrow (no arbitrary font weight, no
/// percent line-height), so we collapse: weight ≥ 600 → strong,
/// `FontFamily::Mono` → monospace, italic → italics. Margins,
/// width, and the rest of the box model are honored by the
/// surrounding layout code, not by `RichText`.
fn apply_computed(rt: egui::RichText, style: &ComputedStyle) -> egui::RichText {
  let mut rt = rt.size(style.font_size).color(rgba_to_color32(style.color));

  if style.font_weight >= 600 {
    rt = rt.strong();
  }

  if matches!(style.font_style, zo_ui_protocol::style::FontStyle::Italic) {
    rt = rt.italics();
  }

  if matches!(style.font_family, FontFamily::Mono) {
    rt = rt.monospace();
  }

  if matches!(
    style.text_decoration,
    zo_ui_protocol::style::TextDecoration::Underline
  ) {
    rt = rt.underline();
  }

  rt
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_named_colors() {
    assert_eq!(
      parse_css_color("cyan"),
      Some(egui::Color32::from_rgb(0, 255, 255))
    );
    assert_eq!(parse_css_color("black"), Some(egui::Color32::BLACK));
    assert_eq!(parse_css_color("white"), Some(egui::Color32::WHITE));
    assert_eq!(parse_css_color("unknown"), None);
  }

  #[test]
  fn parse_hex_3_digit() {
    // #f00 -> rgb(255, 0, 0)
    assert_eq!(
      parse_css_color("#f00"),
      Some(egui::Color32::from_rgb(255, 0, 0))
    );
    // #0ff -> rgb(0, 255, 255) = cyan
    assert_eq!(
      parse_css_color("#0ff"),
      Some(egui::Color32::from_rgb(0, 255, 255))
    );
  }

  #[test]
  fn parse_hex_6_digit() {
    assert_eq!(
      parse_css_color("#b2f5ea"),
      Some(egui::Color32::from_rgb(178, 245, 234))
    );
    assert_eq!(
      parse_css_color("#000000"),
      Some(egui::Color32::from_rgb(0, 0, 0))
    );
    assert_eq!(
      parse_css_color("#ffffff"),
      Some(egui::Color32::from_rgb(255, 255, 255))
    );
  }

  #[test]
  fn parse_css_block_color_and_weight() {
    let mut styles = HashMap::default();

    parse_css_into(&mut styles, "p { color: cyan; font-weight: 800; }\n");

    let p = styles.get("p").expect("should have 'p' entry");

    assert_eq!(p.color, Some(egui::Color32::from_rgb(0, 255, 255)));
    assert!(p.bold, "font-weight 800 should be bold");
  }

  #[test]
  fn parse_css_block_not_bold() {
    let mut styles = HashMap::default();

    parse_css_into(&mut styles, "span { color: red; font-weight: 400; }\n");

    let s = styles.get("span").expect("should have 'span' entry");

    assert_eq!(s.color, Some(egui::Color32::from_rgb(255, 0, 0)));
    assert!(!s.bold, "font-weight 400 should not be bold");
  }

  #[test]
  fn parse_css_scoped_selector_strips_hash() {
    let mut styles = HashMap::default();

    parse_css_into(&mut styles, "p._zo_a3f2 { color: blue; }\n");

    // Should be accessible via "p", not "p._zo_a3f2".
    let p = styles.get("p").expect("scoped selector should map to tag");

    assert_eq!(p.color, Some(egui::Color32::from_rgb(0, 0, 255)));
  }

  #[test]
  fn parse_css_multiple_rules() {
    let mut styles = HashMap::default();

    parse_css_into(
      &mut styles,
      "h1 { color: cyan; }\np { font-weight: bold; }\n",
    );

    let h1 = styles.get("h1").expect("should have 'h1'");
    assert_eq!(h1.color, Some(egui::Color32::from_rgb(0, 255, 255)));

    let p = styles.get("p").expect("should have 'p'");
    assert!(p.bold);
  }
}
