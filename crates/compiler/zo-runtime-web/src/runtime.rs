//! Web runtime for zo applications using wry webview

use crate::renderer::HtmlRenderer;

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
    }
  }

  /// Set UI commands
  pub fn set_commands(&mut self, commands: Vec<UiCommand>) {
    self.commands = commands;
  }

  /// Get the configuration
  pub fn get_config(&self) -> &RuntimeConfig {
    &self.config
  }

  /// Get the commands
  pub fn get_commands(&self) -> &[UiCommand] {
    &self.commands
  }

  /// Run the application with HTML in webview
  pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
    use winit::{
      application::ApplicationHandler,
      event::WindowEvent,
      event_loop::{ActiveEventLoop, EventLoop},
      window::{Window, WindowId},
    };
    use wry::WebView;

    let mut html_renderer = HtmlRenderer::new();
    let html = html_renderer.render_to_html(&self.commands);

    struct App {
      title: String,
      size: (f32, f32),
      html: String,
      window: Option<Window>,
      webview: Option<WebView>,
    }

    impl ApplicationHandler for App {
      fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
          .with_title(&self.title)
          .with_inner_size(winit::dpi::LogicalSize::new(
            self.size.0,
            self.size.1,
          ));

        let window = event_loop.create_window(window_attrs).unwrap();

        let webview = wry::WebViewBuilder::new()
          .with_html(&self.html)
          .build(&window)
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
          WindowEvent::AxisMotion {
            device_id: _,
            axis: _,
            value: _,
          } => {}
          _ => {}
        }
      }
    }

    let event_loop = EventLoop::new()?;
    let mut app = App {
      title: self.config.title,
      size: self.config.size,
      html,
      window: None,
      webview: None,
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
