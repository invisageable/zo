//! Egui-based renderer for UI commands

use crate::loader::image::{ImageLoader, ImageState};

use zo_runtime_render::layout::{LayoutTree, collapse_text};
use zo_runtime_render::render::{EventId, EventPayload, Render, WidgetId};
use zo_ui_protocol::style::{ComputedStyle, FontFamily, Rgba, cascade};
use zo_ui_protocol::{Attr, ElementTag, EventKind, UiCommand};

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
  /// Last-seen `value` attribute per input id. Compared
  /// against the incoming attr each frame; on mismatch,
  /// the program-side state has overwritten the input
  /// (`input_val = ""`) and the renderer's
  /// `text_inputs[id]` must be re-synced. Without this,
  /// the program-side clear silently keeps the stale
  /// user-typed text.
  last_value_attr: HashMap<u32, String>,
  /// Events fired this frame and waiting to be drained
  /// by the runtime, as `(widget_id, kind, payload)`. The
  /// `payload.value` is empty for `Click`-shaped events
  /// and carries the input's current text for the
  /// payload-bearing kinds (`Input`, `Change`, `Submit`).
  pending_events: ThinVec<(u32, EventKind, EventPayload)>,
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

  /// Render commands with an egui UI context. One shared Taffy
  /// solve fixes geometry; egui paints widgets at the solved rects
  /// and routes their input — the row/column flow logic lives in
  /// the solver, identical to every other native target.
  pub fn render_with_ui(&mut self, ui: &mut egui::Ui) {
    // Drain pending image loads before rendering.
    self.image_loader.poll();

    if self.pending_commands.is_empty() {
      return;
    }

    let commands = std::mem::take(&mut self.pending_commands);

    // Pre-scan: parse StyleSheet commands into the styles map
    // (legacy author path — superseded by the shared cascade
    // parser in PLAN_UI_LAYOUT §6).
    self.styles.clear();

    for cmd in &commands {
      if let UiCommand::StyleSheet { css, .. } = cmd {
        parse_css_into(&mut self.styles, css);
      }
    }

    let available = ui.available_size();
    let mut tree = LayoutTree::build(&commands);
    let rects = tree.solve((available.x, available.y));
    let origin = ui.min_rect().min;

    for (idx, rect) in rects {
      let placement = egui::Rect::from_min_size(
        origin + egui::vec2(rect.x, rect.y),
        egui::vec2(rect.width, rect.height),
      );

      self.put_command(ui, &commands, idx, placement);
    }
  }

  /// Paint the placed leaf at `commands[idx]` into `rect` and route
  /// its input. Containers carry no entry, so this only ever sees a
  /// paintable widget (button, image, input, text tag, free text).
  fn put_command(
    &mut self,
    ui: &mut egui::Ui,
    commands: &[UiCommand],
    idx: usize,
    rect: egui::Rect,
  ) {
    match &commands[idx] {
      UiCommand::Element { tag, attrs, .. } => match tag {
        ElementTag::Button => {
          let label = collapse_text(commands, idx + 1);
          let id = attr_num(attrs, "data-id").unwrap_or(0);

          if ui.put(rect, egui::Button::new(label)).clicked() {
            self.state.pending_events.push((
              id,
              EventKind::Click,
              EventPayload::default(),
            ));
          }
        }

        ElementTag::Img => {
          let src = attr_str(attrs, "src").unwrap_or("");

          self.put_image(ui, src, rect);
        }

        ElementTag::Input | ElementTag::Textarea => {
          self.put_input(ui, attrs, rect);
        }

        t if t.is_text_tag() => {
          let content = collapse_text(commands, idx + 1);
          let text = self.styled_text(&content, t);

          ui.put(rect, egui::Label::new(text));
        }

        // Containers are geometry only — nothing to paint at v1.
        _ => {}
      },

      UiCommand::Text(content) => {
        ui.put(rect, egui::Label::new(content.clone()));
      }

      _ => {}
    }
  }

  /// Place a text input at `rect`, syncing program-side value
  /// changes and emitting `@input` / `@submit` events.
  fn put_input(&mut self, ui: &mut egui::Ui, attrs: &[Attr], rect: egui::Rect) {
    let id = attr_num(attrs, "data-id").unwrap_or(0);
    let placeholder = attr_str(attrs, "placeholder").unwrap_or("");
    let value_attr = attr_str(attrs, "value").unwrap_or("").to_string();

    // Sync from the program-side `value` attribute when it has
    // changed since last frame — handles `input_val = ""` after an
    // Add click. User typing doesn't race because the typed text
    // fires `@input` first, and the handler's `input_val = e.value`
    // keeps the attribute in sync with what we already display.
    let resync = self
      .state
      .last_value_attr
      .get(&id)
      .map(|prev| prev != &value_attr)
      .unwrap_or(true);

    if resync {
      self.state.text_inputs.insert(id, value_attr.clone());
      self.state.last_value_attr.insert(id, value_attr);
    }

    let text = self.state.text_inputs.entry(id).or_default();
    let response = ui.put(
      rect,
      egui::TextEdit::singleline(text).hint_text(placeholder),
    );

    if response.changed() {
      // `last_value_attr` tracks the program-side attribute only —
      // writing typed text here makes the next-frame resync wipe
      // `text_inputs[id]`.
      self.state.pending_events.push((
        id,
        EventKind::Input,
        EventPayload::with_value(text.clone()),
      ));
    }

    // `@submit` fires when the user hits Enter while the field has
    // focus. egui's idiom: the field loses focus on the Enter
    // keystroke that submits, so `lost_focus() && enter_pressed` is
    // the gate. Re-grab focus right after so the user can keep
    // typing without re-clicking the field.
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
      self.state.pending_events.push((
        id,
        EventKind::Submit,
        EventPayload::with_value(text.clone()),
      ));

      response.request_focus();
    }
  }

  /// Place an image at `rect`, driving the async loader's state
  /// machine (spinner while decoding, texture once ready).
  fn put_image(&mut self, ui: &mut egui::Ui, src: &str, rect: egui::Rect) {
    let size = rect.size();
    let ctx = ui.ctx().clone();
    let state = self.image_loader.state(src);

    match state {
      ImageState::Pending | ImageState::Loading => {
        ui.put(rect, egui::Spinner::new());
      }
      ImageState::Decoded(_) => {
        let image = match std::mem::replace(state, ImageState::Loading) {
          ImageState::Decoded(img) => img,
          _ => unreachable!(),
        };

        let texture =
          ctx.load_texture(src, image, egui::TextureOptions::default());

        ui.put(
          rect,
          egui::Image::from_texture(&texture).fit_to_exact_size(size),
        );

        *state = ImageState::Ready(texture);
      }
      ImageState::Ready(texture) => {
        ui.put(
          rect,
          egui::Image::from_texture(&*texture).fit_to_exact_size(size),
        );
      }
      ImageState::Failed(error) => {
        ui.put(
          rect,
          egui::Label::new(
            egui::RichText::new(format!("[image error: {error}]"))
              .color(egui::Color32::RED),
          ),
        );
      }
    }
  }

  /// Build the styled `RichText` for a text tag. Resolves the
  /// computed style via the UA cascade so unstyled tags get
  /// browser-like defaults (h1=32px/700, p=16px/400, code=mono,
  /// …); the author `styles` map still overrides on top until the
  /// shared cascade parser feeds it directly (§6).
  fn styled_text(&self, content: &str, tag: &ElementTag) -> egui::RichText {
    let computed = cascade::resolve(tag.as_str(), None, None);
    let mut text = apply_computed(egui::RichText::new(content), &computed);

    if let Some(props) = self.styles.get(tag.as_str()) {
      if let Some(color) = props.color {
        text = text.color(color);
      }

      if props.bold {
        text = text.strong();
      }
    }

    text
  }

  /// Drain events fired this frame; the runtime dispatches
  /// each one against its registered handler.
  pub fn take_pending_events(
    &mut self,
  ) -> ThinVec<(u32, EventKind, EventPayload)> {
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
