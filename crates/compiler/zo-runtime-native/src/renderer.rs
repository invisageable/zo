//! Egui-based renderer for UI commands

use zo_runtime_render::render::{EventId, Render, WidgetId};
use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};

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
}

impl Renderer {
  /// Creates native [`Renderer`] instance.
  pub fn new() -> Self {
    Self {
      state: UiState::default(),
      pending_commands: ThinVec::new(),
      styles: HashMap::default(),
    }
  }

  /// Render commands with an egui UI context
  pub fn render_with_ui(&mut self, ui: &mut egui::Ui) {
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
        UiCommand::BeginContainer { id: _, direction } => {
          idx += 1;

          // Render children in appropriate container
          let end_idx = match direction {
            ContainerDirection::Horizontal => {
              ui.horizontal(|ui| self.render_commands(ui, commands, idx))
                .inner
            }
            ContainerDirection::Vertical => {
              ui.vertical(|ui| self.render_commands(ui, commands, idx))
                .inner
            }
          };

          idx = end_idx;
        }

        UiCommand::EndContainer => {
          // Return to parent container
          return idx + 1;
        }

        UiCommand::Text { content, style } => {
          self.render_styled_text(ui, content, style);

          idx += 1;
        }

        UiCommand::Button { id, content } => {
          if ui.button(content).clicked() {
            println!("Button {id} clicked: {content}");
            self.state.pending_events.push((*id, 0));
          }

          idx += 1;
        }

        UiCommand::TextInput {
          id,
          placeholder,
          value,
        } => {
          let text = self
            .state
            .text_inputs
            .entry(*id)
            .or_insert_with(|| value.clone());

          let response =
            ui.add(egui::TextEdit::singleline(text).hint_text(placeholder));

          if response.changed() {
            println!("Input {id} changed to: {text}");
            self.state.pending_events.push((*id, 1));
          }

          idx += 1;
        }

        UiCommand::Image {
          id: _,
          src,
          width,
          height,
        } => {
          // for now, show placeholder.
          ui.label(format!("[Image: {src} ({width}x{height})]"));

          idx += 1;
        }

        UiCommand::Event { .. } => {
          // events are handled separately.
          idx += 1;
        }

        UiCommand::StyleSheet { .. } => {
          // Native style mapping is post-MVP.
          idx += 1;
        }
      }
    }

    idx
  }

  /// Render text with HTML style + any CSS overrides.
  fn render_styled_text(
    &self,
    ui: &mut egui::Ui,
    content: &str,
    style: &TextStyle,
  ) {
    // Base RichText from the HTML tag type.
    let mut rt = match style {
      TextStyle::Heading1 => egui::RichText::new(content).size(24.0).strong(),
      TextStyle::Heading2 => egui::RichText::new(content).size(20.0).strong(),
      TextStyle::Heading3 => egui::RichText::new(content).size(16.0).strong(),
      TextStyle::Paragraph | TextStyle::Normal => egui::RichText::new(content),
    };

    // Look up the tag name in the styles map.
    let tag = match style {
      TextStyle::Heading1 => "h1",
      TextStyle::Heading2 => "h2",
      TextStyle::Heading3 => "h3",
      TextStyle::Paragraph => "p",
      TextStyle::Normal => "span",
    };

    if let Some(props) = self.styles.get(tag) {
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
