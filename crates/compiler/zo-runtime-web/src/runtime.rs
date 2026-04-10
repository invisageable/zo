//! Web runtime for zo applications using wry webview

use crate::renderer::HtmlRenderer;

use zo_runtime_render::render::EventRegistry;
use zo_ui_protocol::UiCommand;

/// Runtime configuration for web rendering
pub struct RuntimeConfig {
  /// Window title
  pub title: String,
  /// Initial window size
  pub size: (f32, f32),
}

impl Default for RuntimeConfig {
  fn default() -> Self {
    Self {
      title: "zo app".to_string(),
      size: (800.0, 600.0),
    }
  }
}

/// Web runtime for zo applications
pub struct Runtime {
  config: RuntimeConfig,
  commands: Vec<UiCommand>,
  events: EventRegistry,
  /// Shared command buffer for reactive state updates.
  shared_commands: Option<std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>>,
}

impl Runtime {
  /// Create a new web runtime with default configuration
  pub fn new() -> Self {
    Self::with_config(RuntimeConfig::default())
  }

  /// Create a new web runtime with custom configuration
  pub fn with_config(config: RuntimeConfig) -> Self {
    Self {
      config,
      commands: Vec::new(),
      events: EventRegistry::new(),
      shared_commands: None,
    }
  }

  /// Set UI commands
  pub fn set_commands(&mut self, commands: Vec<UiCommand>) {
    self.commands = commands;
  }

  /// Set event handler registry.
  pub fn set_events(&mut self, events: EventRegistry) {
    self.events = events;
  }

  /// Set the shared command buffer (for reactive state).
  pub fn set_shared_commands(
    &mut self,
    shared: std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>,
  ) {
    self.shared_commands = Some(shared);
  }

  /// Run the application with HTML in webview
  pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
    use winit::{
      application::ApplicationHandler,
      event::WindowEvent,
      event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
      window::{Window, WindowId},
    };
    use wry::WebView;

    let mut html_renderer = HtmlRenderer::new();
    let html = html_renderer.render_to_html(&self.commands);

    struct App {
      title: String,
      size: (f32, f32),
      html: String,
      events: EventRegistry,
      commands: Vec<UiCommand>,
      shared_commands: Option<std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>>,
      proxy: EventLoopProxy<String>,
      // WebView must drop before Window.
      webview: Option<WebView>,
      window: Option<Window>,
    }

    impl ApplicationHandler<String> for App {
      fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
          .with_title(&self.title)
          .with_inner_size(winit::dpi::LogicalSize::new(
            self.size.0,
            self.size.1,
          ));

        let window = event_loop.create_window(window_attrs).unwrap();

        // Clone proxy for the IPC handler closure.
        let ipc_proxy = self.proxy.clone();

        let webview = wry::WebViewBuilder::new()
          .with_html(&self.html)
          .with_ipc_handler(move |req| {
            // Forward IPC messages to the event loop.
            let body = req.body().clone();

            ipc_proxy.send_event(body).ok();
          })
          .with_bounds(wry::Rect {
            position: wry::dpi::LogicalPosition::new(0, 0).into(),
            size: wry::dpi::LogicalSize::new(
              self.size.0 as u32,
              self.size.1 as u32,
            )
            .into(),
          })
          .build_as_child(&window)
          .unwrap();

        self.window = Some(window);
        self.webview = Some(webview);
      }

      fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
      ) {
        match event {
          WindowEvent::CloseRequested => event_loop.exit(),
          WindowEvent::Resized(size) => {
            if let Some(wv) = &self.webview {
              let _ = wv.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0, 0).into(),
                size: wry::dpi::LogicalSize::new(size.width, size.height)
                  .into(),
              });
            }
          }
          _ => {}
        }
      }

      fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: String) {
        // IPC message from JS: "click:{widget_id}"
        if let Some(widget_id) = event.strip_prefix("click:") {
          // Find the handler name for this widget.
          let handler_name = self.commands.iter().find_map(|cmd| {
            if let UiCommand::Event {
              widget_id: wid,
              handler,
              ..
            } = cmd
            {
              if wid == widget_id {
                Some(handler.clone())
              } else {
                None
              }
            } else {
              None
            }
          });

          if let Some(name) = handler_name {
            // Dispatch the handler (mutates shared state).
            self.events.dispatch(&name);

            // Read updated commands from the shared buffer.
            let updated_cmds = self
              .shared_commands
              .as_ref()
              .map(|sc| sc.lock().unwrap().clone())
              .unwrap_or_else(|| self.commands.clone());

            // Granular DOM update: diff old vs new commands,
            // update only the changed text nodes.
            if let Some(wv) = &self.webview {
              let mut js = String::new();

              for (idx, (old, new)) in
                self.commands.iter().zip(updated_cmds.iter()).enumerate()
              {
                if old != new
                  && let UiCommand::Text { content, .. } = new
                {
                  js.push_str(&format!(
                    "var e=document.getElementById(\
                     'zo-cmd-{}');\
                     if(e)e.textContent={};",
                    idx,
                    escape_js_string(content),
                  ));
                }
              }

              if !js.is_empty() {
                wv.evaluate_script(&js).ok();
              }

              // Update local commands for next diff.
              self.commands = updated_cmds;
            }
          }
        }
      }
    }

    // Create event loop with user event support for IPC.
    let event_loop = EventLoop::<String>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let mut app = App {
      title: self.config.title,
      size: self.config.size,
      html,
      events: self.events,
      commands: self.commands,
      shared_commands: self.shared_commands,
      proxy,
      webview: None,
      window: None,
    };

    event_loop.run_app(&mut app)?;
    Ok(())
  }
}

impl Default for Runtime {
  fn default() -> Self {
    Self::new()
  }
}

/// Escape a string for use as a JS string literal.
fn escape_js_string(s: &str) -> String {
  let mut out = String::with_capacity(s.len() + 2);

  out.push('"');

  for c in s.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '<' => out.push_str("\\x3c"),
      '>' => out.push_str("\\x3e"),
      _ => out.push(c),
    }
  }

  out.push('"');
  out
}
