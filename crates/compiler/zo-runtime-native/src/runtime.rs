//! Main runtime for zo applications

use crate::renderer::Renderer;

use zo_runtime_render::render::{
  EventRegistry, Render, RuntimeConfig, build_event_map,
};
use zo_ui_protocol::UiCommand;
use zo_ui_protocol::loader::LibraryLoader;

use eframe::egui;

use std::sync::{Arc, Mutex};

/// Main runtime for zo applications
pub struct Runtime {
  config: RuntimeConfig,
  renderer: Renderer,
  loader: LibraryLoader,
  commands: Arc<Mutex<Vec<UiCommand>>>,
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
      commands: Arc::new(Mutex::new(Vec::new())),
      events: EventRegistry::new(),
    }
  }

  /// Load a compiled zo library
  pub fn load_library(
    &mut self,
    path: &str,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let commands = self.loader.load(path)?;

    *self.commands.lock().unwrap() = commands.into();

    Ok(())
  }

  /// Set the shared command buffer.
  pub fn set_shared_commands(&mut self, shared: Arc<Mutex<Vec<UiCommand>>>) {
    self.commands = shared;
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

    eframe::run_native(
      &self.config.title,
      options,
      Box::new(move |cc| {
        crate::theme::style_default(&cc.egui_ctx);

        Ok(Box::new(App {
          renderer: self.renderer,
          commands: self.commands,
          events: self.events,
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
  /// Shared command buffer — handlers update this after
  /// mutating state. Each frame reads the current commands.
  commands: Arc<Mutex<Vec<UiCommand>>>,
  events: EventRegistry,
}

impl eframe::App for App {
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    // eframe 0.34 hands us a raw `Ui`, not a CentralPanel — if we
    // skip wrapping, the viewport clears to whatever the root
    // area paints (black). Own the page background explicitly
    // via `theme::body_frame`, which reads the canonical body
    // gutter from `UA_SHEET`.
    egui::CentralPanel::default()
      .frame(crate::theme::body_frame())
      .show_inside(ui, |ui| {
        let commands = self.commands.lock().unwrap().clone();

        if !commands.is_empty() {
          self.renderer.render(&commands);
        }

        self.renderer.render_with_ui(ui);

        let pending = self.renderer.take_pending_events();

        // Rebuild from the freshly-cloned commands buffer so
        // reactive re-renders that emit new `Event` commands
        // (list items via `apply_list_bindings`, conditional
        // branches, …) get registered the same frame they
        // appear. A startup-only build would silently drop
        // clicks on anything spawned after launch.
        if !pending.is_empty() {
          let event_map = build_event_map(&commands);

          for (widget_id, event_kind, payload) in pending {
            let key = (widget_id.to_string(), event_kind);

            if let Some(handler_name) = event_map.get(&key) {
              self.events.dispatch(handler_name, &payload);
            }
          }
        }
      });
  }
}
