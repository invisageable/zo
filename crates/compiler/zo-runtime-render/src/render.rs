use crate::reactive::DirtySet;

use zo_ui_protocol::{EventKind, UiCommand};

use rustc_hash::FxHashMap as HashMap;
use thin_vec::ThinVec;

use std::sync::{Arc, Mutex};

/// Graphics backend selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
  /// Native egui rendering
  Native,
  /// Web HTML rendering
  Web,
}

/// Shared runtime configuration.
pub struct RuntimeConfig {
  /// Path to a compiled zo library (for `zo build` path).
  pub library_path: Option<String>,
  /// Window title.
  pub title: String,
  /// Initial window size (width, height).
  pub size: (f32, f32),
  /// Graphics backend.
  pub graphics: Graphics,
}

impl Default for RuntimeConfig {
  fn default() -> Self {
    Self {
      library_path: None,
      title: "zo app".to_string(),
      size: (800.0, 600.0),
      graphics: Graphics::Native,
    }
  }
}

/// Runtime-built payload carried into a handler closure when
/// an event fires. Today the only field surfaced to user code
/// is `value` (the text-input current value for `@input`,
/// `@change`, and `@submit`). `@click` and other no-payload
/// events carry an empty string here — the closure's body
/// simply doesn't read it.
#[derive(Clone, Debug, Default)]
pub struct EventPayload {
  pub value: String,
}

impl EventPayload {
  /// Build a payload-bearing event from any string-like
  /// source. Used by both runtimes when forwarding the
  /// input element's current text into the handler.
  pub fn with_value(value: impl Into<String>) -> Self {
    Self {
      value: value.into(),
    }
  }
}

impl std::fmt::Display for EventPayload {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.value)
  }
}

/// Build the `(widget_id, kind) → handler_name` map from a
/// command buffer. Both runtimes call this each time they
/// need to dispatch an event so reactive re-renders that
/// introduce new `Event` commands (e.g. list items emitted
/// by `apply_list_bindings`) are picked up immediately. The
/// tuple key is what lets a single element bind multiple
/// kinds (`<input @input={a} @submit={b}/>`) without one
/// overwriting the other.
pub fn build_event_map(
  commands: &[UiCommand],
) -> HashMap<(String, EventKind), String> {
  let mut map = HashMap::default();

  for cmd in commands {
    if let UiCommand::Event {
      widget_id,
      event_kind,
      handler,
      ..
    } = cmd
    {
      map.insert((widget_id.clone(), *event_kind), handler.clone());
    }
  }

  map
}

/// Event handler callback. Receives the runtime-built payload
/// for the event that fired. Click handlers ignore the payload;
/// input/change handlers read `payload.value`.
pub type EventHandler = Box<dyn Fn(&EventPayload) + Send>;

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

  pub fn dispatch(&self, name: &str, payload: &EventPayload) -> bool {
    if let Some(handler) = self.handlers.get(name) {
      handler(payload);
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
  /// Array of strings — backs `mut []str` state for list
  /// rendering (`<X>{arr.map(fn(t) => ...)}</X>`). The
  /// `display()` form is the formatter's `[…]` view; for
  /// list rendering the runtime walks the inner Vec
  /// directly via `as_strs`.
  Strs(Vec<String>),
}

impl StateValue {
  /// Display the value as a string (for template rendering).
  pub fn display(&self) -> String {
    match self {
      Self::Int(v) => v.to_string(),
      Self::Float(v) => v.to_string(),
      Self::Bool(v) => v.to_string(),
      Self::Str(v) => v.clone(),
      Self::Strs(v) => format!("{v:?}"),
    }
  }

  /// Borrow the inner string array if this is a `Strs`.
  /// Used by the list-rendering path.
  pub fn as_strs(&self) -> Option<&[String]> {
    match self {
      Self::Strs(v) => Some(v),
      _ => None,
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
      Self::Strs(v) => write!(f, "{v:?}"),
    }
  }
}

/// A shared mutable state cell. Thread-safe for use across
/// handler closures and the render loop.
///
/// When wired with a dirty channel (`with_dirty`), every write
/// marks the cell's `slot` in a shared [`DirtySet`] — the
/// `zo run` mirror of the compiled path's `zo_state_set`. Cells
/// built with `new` (unit tests, static templates) skip
/// tracking: their `dirty` channel is `None`.
#[derive(Clone, Debug)]
pub struct StateCell {
  value: Arc<Mutex<StateValue>>,
  /// `(shared dirty set, this cell's slot)`. `None` disables
  /// tracking.
  dirty: Option<(Arc<Mutex<DirtySet>>, u32)>,
}

impl StateCell {
  /// Create a new state cell with an initial value and no dirty
  /// tracking.
  pub fn new(value: StateValue) -> Self {
    Self {
      value: Arc::new(Mutex::new(value)),
      dirty: None,
    }
  }

  /// Create a state cell that marks `slot` in `dirty` on every
  /// write. The driver wires each reactive cell this way so a
  /// handler's `cell.set(...)` records exactly which slots
  /// changed.
  pub fn with_dirty(
    value: StateValue,
    dirty: Arc<Mutex<DirtySet>>,
    slot: u32,
  ) -> Self {
    Self {
      value: Arc::new(Mutex::new(value)),
      dirty: Some((dirty, slot)),
    }
  }

  /// Read the current value.
  pub fn get(&self) -> StateValue {
    self.value.lock().unwrap().clone()
  }

  /// Set a new value, marking the cell's slot dirty.
  pub fn set(&self, value: StateValue) {
    *self.value.lock().unwrap() = value;
    self.mark_dirty();
  }

  /// Borrow the inner string array (read-only) under the
  /// cell's lock, returning the closure's result. Used by
  /// the list-rendering path to avoid cloning the whole
  /// `Vec<String>` per event. Returns `None` when the cell
  /// is some other variant.
  pub fn with_strs<R>(&self, f: impl FnOnce(&[String]) -> R) -> Option<R> {
    let guard = self.value.lock().unwrap();

    match &*guard {
      StateValue::Strs(v) => Some(f(v)),
      _ => None,
    }
  }

  /// True when the cell holds a `Strs` variant. Cheap
  /// peek alternative to `get()` for the evaluator's
  /// param-binding decision (lets us pick `StrArrRef`
  /// without cloning the `Vec`).
  pub fn is_strs(&self) -> bool {
    matches!(*self.value.lock().unwrap(), StateValue::Strs(_))
  }

  /// Apply a mutation function to the value, marking the cell's
  /// slot dirty.
  pub fn mutate(&self, f: impl FnOnce(&mut StateValue)) {
    f(&mut self.value.lock().unwrap());
    self.mark_dirty();
  }

  /// Mark this cell's slot in the shared dirty set, when wired.
  fn mark_dirty(&self) {
    if let Some((dirty, slot)) = &self.dirty {
      dirty.lock().unwrap().mark(*slot);
    }
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

  #[test]
  fn state_cell_with_dirty_marks_slot() {
    let dirty = Arc::new(Mutex::new(DirtySet::with_capacity(8)));
    let cell = StateCell::with_dirty(StateValue::Int(0), dirty.clone(), 3);

    cell.set(StateValue::Int(42));
    cell.mutate(|v| {
      if let StateValue::Int(n) = v {
        *n += 1;
      }
    });

    let mut drained = Vec::new();
    dirty.lock().unwrap().drain_into(&mut drained);

    // Both writes mark slot 3; the set de-duplicates.
    assert_eq!(drained, vec![3]);
  }

  #[test]
  fn state_cell_new_skips_tracking() {
    // A `new` cell has no dirty channel: writes still work, no
    // panic, nothing to drain.
    let cell = StateCell::new(StateValue::Int(0));

    cell.set(StateValue::Int(1));

    assert_eq!(cell.get(), StateValue::Int(1));
  }
}
