//! Web runtime for zo applications using wry webview

use crate::renderer::HtmlRenderer;

use zo_runtime_render::render::{EventRegistry, RuntimeConfig};
use zo_ui_protocol::UiCommand;

/// Web runtime for zo applications
pub struct Runtime {
  config: RuntimeConfig,
  events: EventRegistry,
  /// Shared command buffer — single source of truth.
  commands: std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>,
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
      events: EventRegistry::new(),
      commands: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    }
  }

  /// Set the shared command buffer.
  pub fn set_shared_commands(
    &mut self,
    shared: std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>,
  ) {
    self.commands = shared;
  }

  /// Set event handler registry.
  pub fn set_events(&mut self, events: EventRegistry) {
    self.events = events;
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

    let commands = self.commands.lock().unwrap().clone();
    let mut html_renderer = HtmlRenderer::new();
    let html = html_renderer.render_to_html(&commands);

    struct App {
      title: String,
      size: (f32, f32),
      html: String,
      events: EventRegistry,
      /// Current commands for diffing.
      commands: Vec<UiCommand>,
      /// Shared buffer — handlers write here.
      shared: std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>,
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

        // Serve the document and local image assets through a
        // custom `zo://` protocol. Loading the HTML via custom
        // protocol (instead of `with_html`) gives the page a
        // stable `zo://localhost` origin — same-origin requests
        // for `zo://localhost/<abs-path>` assets then succeed
        // where bare `file://` URLs would be blocked.
        let html = self.html.clone();

        let webview = wry::WebViewBuilder::new()
          .with_custom_protocol("zo".into(), move |_id, request| {
            serve_asset(&html, request)
          })
          .with_url("zo://localhost/")
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
            self.events.dispatch(&name);

            // Read updated commands from shared buffer.
            let updated = self.shared.lock().unwrap().clone();

            // Granular DOM update: only changed text.
            if let Some(wv) = &self.webview {
              let mut js = String::new();

              for (idx, (old, new)) in
                self.commands.iter().zip(updated.iter()).enumerate()
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

              self.commands = updated;
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
      commands,
      shared: self.commands,
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

/// Serve a request on the `zo://` custom protocol.
///
/// - `/` (or empty path) → the generated HTML document.
/// - any other path → the file on disk at that absolute path.
fn serve_asset(
  html: &str,
  request: wry::http::Request<Vec<u8>>,
) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
  use wry::http::{Response, header::CONTENT_TYPE};

  let path = request.uri().path();

  if path.is_empty() || path == "/" {
    return Response::builder()
      .header(CONTENT_TYPE, "text/html")
      .body(html.as_bytes().to_vec().into())
      .unwrap();
  }

  // Custom protocol strips the scheme+host; `path` is the
  // absolute filesystem path with a single leading `/`.
  let fs_path: &std::path::Path = path.as_ref();

  match std::fs::read(fs_path) {
    Ok(bytes) => Response::builder()
      .header(CONTENT_TYPE, mime_from_path(path))
      .body(bytes.into())
      .unwrap(),
    Err(_) => Response::builder()
      .status(404)
      .header(CONTENT_TYPE, "text/plain")
      .body(Vec::<u8>::new().into())
      .unwrap(),
  }
}

/// Infer a MIME type from a file extension.
fn mime_from_path(path: &str) -> &'static str {
  let ext = std::path::Path::new(path)
    .extension()
    .and_then(|e| e.to_str())
    .map(|e| e.to_ascii_lowercase());

  match ext.as_deref() {
    Some("html" | "htm") => "text/html",
    Some("js") => "text/javascript",
    Some("css") => "text/css",
    Some("jpg" | "jpeg") => "image/jpeg",
    Some("png") => "image/png",
    Some("gif") => "image/gif",
    Some("webp") => "image/webp",
    Some("svg") => "image/svg+xml",
    _ => "application/octet-stream",
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
