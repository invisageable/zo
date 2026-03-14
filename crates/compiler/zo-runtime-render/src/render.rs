use zo_ui_protocol::UiCommand;

use rustc_hash::FxHashMap;
use thin_vec::ThinVec;

/// Graphics backend selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
  /// Native egui rendering
  Native,
  /// Web HTML rendering
  Web,
}

/// Event handler callback.
pub type EventHandler = Box<dyn Fn() + Send>;

/// Registry mapping handler names to callable functions.
/// Built by the driver from SIR, consumed by runtimes.
pub struct EventRegistry {
  handlers: FxHashMap<String, EventHandler>,
}
impl EventRegistry {
  pub fn new() -> Self {
    Self {
      handlers: FxHashMap::default(),
    }
  }

  pub fn register(&mut self, name: String, handler: EventHandler) {
    self.handlers.insert(name, handler);
  }

  pub fn dispatch(&self, name: &str) -> bool {
    if let Some(handler) = self.handlers.get(name) {
      handler();
      true
    } else {
      false
    }
  }

  pub fn has(&self, name: &str) -> bool {
    self.handlers.contains_key(name)
  }
}
impl Default for EventRegistry {
  fn default() -> Self {
    Self::new()
  }
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
