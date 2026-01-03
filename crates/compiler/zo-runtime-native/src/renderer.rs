//! Egui-based renderer for UI commands

use zo_runtime_render::render::{EventId, Render, WidgetId};
use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};

use eframe::egui;
use thin_vec::ThinVec;

/// State for managing UI elements
#[derive(Default)]
pub struct UiState {
  /// Text input values indexed by ID
  text_inputs: std::collections::HashMap<u32, String>,
  /// Button click events to send back
  pending_events: ThinVec<(u32, u32)>, // (widget_id, event_type)
}

/// Egui-based renderer for zo UI commands
pub struct Renderer {
  state: UiState,
  /// Store commands for rendering in the egui context
  pending_commands: ThinVec<UiCommand>,
}

impl Renderer {
  pub fn new() -> Self {
    Self {
      state: UiState::default(),
      pending_commands: ThinVec::new(),
    }
  }

  /// Render commands with an egui UI context
  pub fn render_with_ui(&mut self, ui: &mut egui::Ui) {
    if !self.pending_commands.is_empty() {
      let commands = std::mem::take(&mut self.pending_commands);
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
          self.render_text(ui, content, style);
          idx += 1;
        }

        UiCommand::Button { id, content } => {
          if ui.button(content).clicked() {
            // Record button click event
            self.state.pending_events.push((*id, 0)); // 0 = Click event
            println!("Button {} clicked: {}", id, content);
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
            self.state.pending_events.push((*id, 1)); // 1 = Change event
            println!("Input {id} changed to: {text}");
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
      }
    }

    idx
  }

  /// Render text with the appropriate style
  fn render_text(&self, ui: &mut egui::Ui, content: &str, style: &TextStyle) {
    match style {
      TextStyle::Heading1 => {
        ui.heading(content);
      }
      TextStyle::Heading2 => {
        ui.label(egui::RichText::new(content).size(20.0).strong());
      }
      TextStyle::Heading3 => {
        ui.label(egui::RichText::new(content).size(16.0).strong());
      }
      TextStyle::Paragraph => {
        ui.label(content);
      }
      TextStyle::Normal => {
        ui.label(content);
      }
    }
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
