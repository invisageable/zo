//! Main runtime for zo applications

use crate::renderer::Renderer;

use zo_runtime_render::render::{EventRegistry, Render};
use zo_ui_protocol::UiCommand;
use zo_ui_protocol::loader::LibraryLoader;

use eframe::egui;
use rustc_hash::FxHashMap as HashMap;

/// Runtime configuration
pub struct RuntimeConfig {
  /// Path to the compiled zo library
  pub library_path: Option<String>,
  /// Window title
  pub title: String,
  /// Initial window size
  pub size: (f32, f32),
}

impl Default for RuntimeConfig {
  fn default() -> Self {
    Self {
      library_path: None,
      title: "zo app".to_string(),
      size: (800.0, 600.0),
    }
  }
}

/// Main runtime for zo applications
pub struct Runtime {
  config: RuntimeConfig,
  renderer: Renderer,
  loader: LibraryLoader,
  commands: Vec<UiCommand>,
  events: EventRegistry,
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
      renderer: Renderer::new(),
      loader: LibraryLoader::new(),
      commands: Vec::new(),
      events: EventRegistry::new(),
    }
  }

  /// Load a compiled zo library
  pub fn load_library(
    &mut self,
    path: &str,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let commands = self.loader.load(path)?;
    self.commands = commands.into();
    Ok(())
  }

  /// Set UI commands directly (for testing)
  pub fn set_commands(&mut self, commands: Vec<UiCommand>) {
    self.commands = commands;
  }

  /// Set event handler registry.
  pub fn set_events(&mut self, events: EventRegistry) {
    self.events = events;
  }

  /// Run the application with egui
  pub fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = self.config.library_path.clone() {
      self.load_library(&path)?;
    }

    let options = eframe::NativeOptions {
      viewport: egui::ViewportBuilder::default()
        .with_inner_size([self.config.size.0, self.config.size.1])
        .with_title(&self.config.title),
      ..Default::default()
    };

    let commands = self.commands.clone();

    // Build widget_id → handler_name map from Event commands
    let mut event_map: HashMap<String, String> = HashMap::default();
    for cmd in &commands {
      if let UiCommand::Event {
        widget_id, handler, ..
      } = cmd
      {
        event_map.insert(widget_id.clone(), handler.clone());
      }
    }

    eframe::run_native(
      &self.config.title,
      options,
      Box::new(move |_cc| {
        Ok(Box::new(App {
          renderer: self.renderer,
          commands,
          events: self.events,
          event_map,
        }))
      }),
    )
    .map_err(|e| format!("Failed to run application: {e}"))?;

    Ok(())
  }
}

impl Default for Runtime {
  fn default() -> Self {
    Self::new()
  }
}

/// Egui application wrapper
struct App {
  renderer: Renderer,
  commands: Vec<UiCommand>,
  events: EventRegistry,
  /// Maps widget_id → handler_name
  event_map: HashMap<String, String>,
}

impl eframe::App for App {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    if !self.commands.is_empty() {
      self.renderer.render(&self.commands);
    }

    egui::CentralPanel::default().show(ctx, |ui| {
      self.renderer.render_with_ui(ui);

      // Dispatch pending events to zo handlers
      let pending = self.renderer.take_pending_events();
      for (widget_id, _event_kind) in pending {
        let wid = widget_id.to_string();
        if let Some(handler_name) = self.event_map.get(&wid) {
          self.events.dispatch(handler_name);
        }
      }
    });
  }
}
