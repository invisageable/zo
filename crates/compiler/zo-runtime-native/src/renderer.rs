//! Egui-based renderer for UI commands

use crate::loader::image::{ImageLoader, ImageState};

use zo_runtime_render::render::{EventId, Render, WidgetId};
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

      self.render_commands(ui, &commands, 0);
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
        if self_closing {
          children_start
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

  /// Render styled text for an `ElementTag` text tag. Mirrors
  /// `render_styled_text` for the legacy `UiCommand::Text` path.
  fn render_styled_text_for_tag(
    &self,
    ui: &mut egui::Ui,
    content: &str,
    tag: &ElementTag,
  ) {
    let mut rt = match tag {
      ElementTag::H1 => egui::RichText::new(content).size(24.0).strong(),
      ElementTag::H2 => egui::RichText::new(content).size(20.0).strong(),
      ElementTag::H3 => egui::RichText::new(content).size(16.0).strong(),
      _ => egui::RichText::new(content),
    };

    // Look up the tag name in the styles map.
    if let Some(props) = self.styles.get(tag.as_str()) {
      if let Some(c) = props.color {
        rt = rt.color(c);
      }

      if props.bold {
        rt = rt.strong();
      }
    }

    ui.label(rt);
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
      UiCommand::Element { self_closing, .. } => {
        if !self_closing {
          depth += 1;
        }
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

/// Return the index just after the matching `EndElement` for the
/// element whose children begin at `start`. If no matching
/// `EndElement` is found, return `commands.len()`.
fn skip_to_end_element(commands: &[UiCommand], start: usize) -> usize {
  let mut depth: usize = 0;
  let mut idx = start;

  while idx < commands.len() {
    match &commands[idx] {
      UiCommand::Element { self_closing, .. } => {
        if !self_closing {
          depth += 1;
        }
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
