use zo_ui_protocol::UiCommand;

use rustc_hash::FxHashMap as HashMap;
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
  handlers: HashMap<String, EventHandler>,
}

impl EventRegistry {
  pub fn new() -> Self {
    Self {
      handlers: HashMap::default(),
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

// === Reactive State ===

/// A runtime value for reactive state slots.
#[derive(Clone, Debug, PartialEq)]
pub enum StateValue {
  Int(i64),
  Float(f64),
  Bool(bool),
  Str(String),
}

impl StateValue {
  /// Display the value as a string (for template rendering).
  pub fn display(&self) -> String {
    match self {
      Self::Int(v) => v.to_string(),
      Self::Float(v) => v.to_string(),
      Self::Bool(v) => v.to_string(),
      Self::Str(v) => v.clone(),
    }
  }
}

impl std::fmt::Display for StateValue {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Int(v) => write!(f, "{v}"),
      Self::Float(v) => write!(f, "{v}"),
      Self::Bool(v) => write!(f, "{v}"),
      Self::Str(v) => write!(f, "{v}"),
    }
  }
}

/// A shared mutable state cell. Thread-safe for use across
/// handler closures and the render loop.
#[derive(Clone, Debug)]
pub struct StateCell(std::sync::Arc<std::sync::Mutex<StateValue>>);

impl StateCell {
  /// Create a new state cell with an initial value.
  pub fn new(value: StateValue) -> Self {
    Self(std::sync::Arc::new(std::sync::Mutex::new(value)))
  }

  /// Read the current value.
  pub fn get(&self) -> StateValue {
    self.0.lock().unwrap().clone()
  }

  /// Set a new value.
  pub fn set(&self, value: StateValue) {
    *self.0.lock().unwrap() = value;
  }

  /// Apply a mutation function to the value.
  pub fn mutate(&self, f: impl FnOnce(&mut StateValue)) {
    f(&mut self.0.lock().unwrap());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_state_value_display() {
    assert_eq!(StateValue::Int(42).display(), "42");
    assert_eq!(StateValue::Float(2.5).display(), "2.5");
    assert_eq!(StateValue::Bool(true).display(), "true");
    assert_eq!(StateValue::Str("hello".into()).display(), "hello",);
  }

  #[test]
  fn test_state_cell_get_set() {
    let cell = StateCell::new(StateValue::Int(0));

    assert_eq!(cell.get(), StateValue::Int(0));

    cell.set(StateValue::Int(42));
    assert_eq!(cell.get(), StateValue::Int(42));
  }

  #[test]
  fn test_state_cell_mutate() {
    let cell = StateCell::new(StateValue::Int(5));

    cell.mutate(|v| {
      if let StateValue::Int(n) = v {
        *n -= 1;
      }
    });

    assert_eq!(cell.get(), StateValue::Int(4));
  }

  #[test]
  fn test_state_cell_shared() {
    let cell = StateCell::new(StateValue::Int(0));
    let cell2 = cell.clone();

    cell.set(StateValue::Int(10));
    assert_eq!(cell2.get(), StateValue::Int(10));
  }

  #[test]
  fn test_state_cell_thread_safe() {
    let cell = StateCell::new(StateValue::Int(0));
    let cell2 = cell.clone();

    let handle = std::thread::spawn(move || {
      cell2.set(StateValue::Int(99));
    });

    handle.join().unwrap();
    assert_eq!(cell.get(), StateValue::Int(99));
  }
}
