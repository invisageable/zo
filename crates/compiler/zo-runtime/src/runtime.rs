//! Main runtime dispatcher for zo applications

use zo_runtime_render::render::{EventRegistry, Graphics, RuntimeConfig};
use zo_ui_protocol::UiCommand;

use std::sync::{Arc, Mutex};

/// Main runtime dispatcher for zo applications
pub struct Runtime {
  config: RuntimeConfig,
  events: EventRegistry,
  /// Shared command buffer. The single source of truth for
  /// UI commands — both initial render and reactive updates.
  commands: Arc<Mutex<Vec<UiCommand>>>,
}

impl Runtime {
  /// Create a new runtime with default configuration
  pub fn new() -> Self {
    Self::with_config(RuntimeConfig::default())
  }

  /// Create a new runtime with custom configuration
  pub fn with_config(config: RuntimeConfig) -> Self {
    Self {
      config,
      events: EventRegistry::new(),
      commands: Arc::new(Mutex::new(Vec::new())),
    }
  }

  /// Set UI commands.
  pub fn set_commands(&mut self, commands: Vec<UiCommand>) {
    *self.commands.lock().unwrap() = commands;
  }

  /// Set the event handler registry.
  pub fn set_events(&mut self, events: EventRegistry) {
    self.events = events;
  }

  /// Get a shared handle to the command buffer.
  pub fn shared_commands(&self) -> Arc<Mutex<Vec<UiCommand>>> {
    self.commands.clone()
  }

  /// Run the application with the configured graphics backend
  pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
    match self.config.graphics {
      Graphics::Native => {
        let mut native_runtime =
          zo_runtime_native::runtime::Runtime::with_config(self.config);

        native_runtime.set_shared_commands(self.commands);
        native_runtime.set_events(self.events);
        native_runtime.run()
      }
      Graphics::Web => {
        let mut web_runtime = zo_runtime_web::Runtime::with_config(self.config);

        web_runtime.set_shared_commands(self.commands);
        web_runtime.set_events(self.events);
        web_runtime.run()
      }
    }
  }
}

impl Default for Runtime {
  fn default() -> Self {
    Self::new()
  }
}
