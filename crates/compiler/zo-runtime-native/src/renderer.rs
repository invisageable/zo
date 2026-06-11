//! Egui-based renderer for UI commands

use crate::loader::image::{ImageLoader, ImageState};

use zo_runtime_render::layout::InteractionAuthors;
use zo_runtime_render::layout::{LayoutTree, collapse_text};
use zo_runtime_render::render::{EventId, EventPayload, Render, WidgetId};
use zo_ui_protocol::style::{
  ComputedStyle, FontFamily, Rgba, StylePatch, cascade,
};
use zo_ui_protocol::{Attr, ElementTag, EventKind, UiCommand};

use eframe::egui;
use rustc_hash::FxHashMap as HashMap;
use thin_vec::ThinVec;

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

    let available = ui.available_size();
    let mut tree = LayoutTree::build(&commands);
    let rects = tree.solve((available.x, available.y));
    let styles = tree.styles();
    let authors = tree.authors();
    let interactions = tree.interactions();
    let origin = ui.min_rect().min;

    for (i, (idx, rect)) in rects.iter().enumerate() {
      let placement = egui::Rect::from_min_size(
        origin + egui::vec2(rect.x, rect.y),
        egui::vec2(rect.width, rect.height),
      );

      // Interaction states resolve at paint time: immediate mode
      // re-runs this every frame, so overlaying the matching state
      // patch on the static author patch is the whole mechanism —
      // no retained state machine. Focus needs the widget id egui
      // assigns during `put`, which doesn't exist yet here; it
      // lands with the form-controls phase.
      let (effective_style, effective_author) = resolve_interaction_state(
        ui,
        &commands[*idx],
        placement,
        (&styles[i], &authors[i]),
        &interactions[i],
      );

      self.put_command(
        ui,
        &commands,
        *idx,
        placement,
        &effective_style,
        &effective_author,
      );
    }
  }

  /// Paint the placed leaf at `commands[idx]` into `rect` and route
  /// its input. Containers carry no entry, so this only ever sees a
  /// paintable widget (button, image, input, text tag, free text).
  /// `style` is the resolved cascade; `author` says which colours the
  /// stylesheet declared, so undeclared widgets keep egui's defaults.
  fn put_command(
    &mut self,
    ui: &mut egui::Ui,
    commands: &[UiCommand],
    idx: usize,
    rect: egui::Rect,
    style: &ComputedStyle,
    author: &StylePatch,
  ) {
    match &commands[idx] {
      UiCommand::Element { tag, attrs, .. } => match tag {
        ElementTag::Button => {
          let id = attr_num(attrs, "data-id").unwrap_or(0);
          let mut label = egui::RichText::new(collapse_text(commands, idx + 1));

          // Declared `color` paints the title; otherwise egui's
          // default button text colour stands.
          if author.color.is_some() {
            label = label.color(rgba_to_color32(style.color));
          }

          let mut button = egui::Button::new(label);

          if author.background.is_some() {
            button = button.fill(rgba_to_color32(style.background));
          }

          if author.border_width.is_some() || author.border_color.is_some() {
            button = button.stroke(egui::Stroke::new(
              style.border_width.max(1.0),
              rgba_to_color32(style.border_color),
            ));
          }

          if author.border_radius.is_some() {
            button = button.corner_radius(egui::CornerRadius::same(
              style.border_radius as u8,
            ));
          }

          if ui.put(rect, button).clicked() {
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
          let text = apply_computed(egui::RichText::new(content), style);

          ui.put(rect, egui::Label::new(text));
        }

        // Containers paint their surface (shadow, fill, border)
        // behind the children the layout placed after them.
        _ => {
          paint_surface(ui, rect, style, author);
        }
      },

      UiCommand::Text(content) => {
        let text = apply_computed(egui::RichText::new(content), style);

        ui.put(rect, egui::Label::new(text));
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
/// Overlay the interaction-state patches matching the element's
/// current pointer state onto its author patch, re-cascading when
/// anything applied. Returns the untouched base for elements
/// without state rules (the common case — one `is_empty` check).
fn resolve_interaction_state(
  ui: &egui::Ui,
  command: &UiCommand,
  rect: egui::Rect,
  base: (&ComputedStyle, &StylePatch),
  interactions: &InteractionAuthors,
) -> (ComputedStyle, StylePatch) {
  let (style, author) = base;

  if interactions.is_empty() {
    return (*style, *author);
  }

  let UiCommand::Element { tag, attrs, .. } = command else {
    return (*style, *author);
  };

  let hovered = ui.rect_contains_pointer(rect);
  let pressed = hovered && ui.input(|input| input.pointer.primary_down());
  let disabled = attr_flag(attrs, "disabled");

  let mut effective = *author;
  let mut applied = false;

  // Overlay order is specificity-by-state: hover under active
  // (a pressed pointer is also hovering), disabled last — a
  // disabled control must not light up under the pointer.
  if hovered && let Some(patch) = &interactions.hover {
    effective.overlay(patch);
    applied = true;
  }

  if pressed && let Some(patch) = &interactions.active {
    effective.overlay(patch);
    applied = true;
  }

  if disabled && let Some(patch) = &interactions.disabled {
    effective = *author;
    effective.overlay(patch);
    applied = true;
  }

  if !applied {
    return (*style, *author);
  }

  let resolved = cascade::resolve(tag.as_str(), Some(&effective), None);

  (resolved, effective)
}

/// True when a boolean attribute (`<button disabled>`) is present.
fn attr_flag(attrs: &[Attr], name: &str) -> bool {
  attrs.iter().any(|attr| attr.name() == name)
}

/// Paint a container's surface at its solved rect: shadow first
/// (behind), then the background fill, then the border stroke.
/// Each layer paints only when the stylesheet declared it, so
/// undeclared containers stay invisible exactly as before.
fn paint_surface(
  ui: &egui::Ui,
  rect: egui::Rect,
  style: &ComputedStyle,
  author: &StylePatch,
) {
  let radius = egui::CornerRadius::same(style.border_radius as u8);
  let painter = ui.painter();

  if let Some(shadow) = style.box_shadow {
    let egui_shadow = egui::epaint::Shadow {
      offset: [shadow.offset_x as i8, shadow.offset_y as i8],
      blur: shadow.blur as u8,
      spread: 0,
      color: rgba_to_color32(shadow.color),
    };

    painter.add(egui_shadow.as_shape(rect, radius));
  }

  if author.background.is_some() {
    painter.rect_filled(rect, radius, rgba_to_color32(style.background));
  }

  if style.border_width > 0.0 {
    painter.rect_stroke(
      rect,
      radius,
      egui::Stroke::new(
        style.border_width,
        rgba_to_color32(style.border_color),
      ),
      egui::StrokeKind::Inside,
    );
  }
}

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
