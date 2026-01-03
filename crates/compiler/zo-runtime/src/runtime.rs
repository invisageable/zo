//! Main runtime dispatcher for zo applications

use zo_runtime_render::render::Graphics;
use zo_ui_protocol::UiCommand;

/// Runtime configuration
pub struct RuntimeConfig {
  /// Path to the compiled zo library
  pub library_path: Option<String>,
  /// Window title
  pub title: String,
  /// Initial window size
  pub size: (f32, f32),
  /// Graphics backend
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

/// Main runtime dispatcher for zo applications
pub struct Runtime {
  config: RuntimeConfig,
  commands: Vec<UiCommand>,
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
      commands: Vec::new(),
    }
  }

  /// Set UI commands directly (for testing)
  pub fn set_commands(&mut self, commands: Vec<UiCommand>) {
    self.commands = commands;
  }

  /// Run the application with the configured graphics backend
  pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
    match self.config.graphics {
      Graphics::Native => {
        let native_config = zo_runtime_native::runtime::RuntimeConfig {
          library_path: self.config.library_path,
          title: self.config.title,
          size: self.config.size,
        };

        let mut native_runtime =
          zo_runtime_native::runtime::Runtime::with_config(native_config);
        native_runtime.set_commands(self.commands);
        native_runtime.run()
      }
      Graphics::Web => {
        let web_config = zo_runtime_web::RuntimeConfig {
          title: self.config.title,
          size: self.config.size,
        };

        let mut web_runtime = zo_runtime_web::Runtime::with_config(web_config);
        web_runtime.set_commands(self.commands);
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
