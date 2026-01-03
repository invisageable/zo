use zo_ui_protocol::UiCommand;

use thin_vec::ThinVec;

/// Graphics backend selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
  /// Native egui rendering
  Native,
  /// Web HTML rendering
  Web,
}

/// Represents a [`WidgetId`] instance.
pub struct WidgetId(pub u32);

/// Represents a [`EventId`] instance.
pub struct EventId(pub u32);

/// Represents a [`Render`] trait.
pub trait Render {
  /// Render the UI commands to the target platform
  fn render(&mut self, commands: &[UiCommand]);

  /// Handles events from the UI.
  fn handle_event(
    &mut self,
    widget_id: &WidgetId,
    event_id: &EventId,
    event_data: ThinVec<u8>,
  );

  /// Initializes the renderer — *called once at startup*.
  fn init(&mut self) -> Result<(), Box<dyn std::error::Error>>;

  /// Cleanup resources — *called on shutdown*.
  fn cleanup(&mut self);
}
