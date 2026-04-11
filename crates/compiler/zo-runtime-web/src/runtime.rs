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

            // Granular DOM update: walk the command diff and
            // emit a targeted JS patch for each changed
            // command. All elements carry a uniform
            // `data-zo-cmd="{idx}"` id, so the same selector
            // works for text, attributes, and future element
            // types.
            if let Some(wv) = &self.webview {
              let js = build_patch_js(&self.commands, &updated);

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
  // On Windows, drive-letter paths reach us as `/C:/foo.png`
  // — strip the leading `/` so `std::fs::read` sees a valid
  // `C:/foo.png` path.
  let fs_path_str = uri_path_to_fs(path);
  let fs_path: &std::path::Path = fs_path_str.as_ref();

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

/// Convert a URI path (`/C:/foo.png` or `/tmp/foo.png`) to a
/// filesystem path string. On Windows, a leading `/` followed
/// by a drive letter is stripped (`/C:/foo.png` → `C:/foo.png`).
/// Unix paths pass through unchanged.
fn uri_path_to_fs(uri_path: &str) -> String {
  #[cfg(windows)]
  {
    // Match `/X:/...` where X is an ASCII letter.
    let bytes = uri_path.as_bytes();

    if bytes.len() >= 4
      && bytes[0] == b'/'
      && bytes[1].is_ascii_alphabetic()
      && bytes[2] == b':'
      && (bytes[3] == b'/' || bytes[3] == b'\\')
    {
      return uri_path[1..].to_string();
    }
  }

  uri_path.to_string()
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

/// Build the JS patch string that brings the webview's DOM in
/// sync with `new` given the previously-rendered `old`
/// commands. Emits one `querySelector` + `textContent`
/// assignment per changed `UiCommand::Text`, and one
/// `querySelector` + `setAttribute` call per changed attribute
/// on a `UiCommand::Element`. Returns an empty string when
/// nothing changed. Extracted as a pure function so the logic
/// is unit-testable without a live webview.
fn build_patch_js(old: &[UiCommand], new: &[UiCommand]) -> String {
  let mut js = String::new();

  for (idx, (a, b)) in old.iter().zip(new.iter()).enumerate() {
    if a == b {
      continue;
    }

    match (a, b) {
      // PCDATA node changed — replace element's text content
      // via the uniform `[data-zo-cmd]` selector.
      (_, UiCommand::Text(content)) => {
        js.push_str(&format!(
          "var e=document.querySelector(\
           '[data-zo-cmd=\"{idx}\"]');\
           if(e)e.textContent={};",
          escape_js_string(content),
        ));
      }
      // Element attrs changed — diff pairwise and emit
      // `setAttribute` for each updated attr. Works uniformly
      // for reactive `Attr::Dynamic` updates (the initial
      // value is re-stringified by the driver's patch loop)
      // and any other attribute source.
      (
        UiCommand::Element {
          attrs: old_attrs, ..
        },
        UiCommand::Element {
          attrs: new_attrs, ..
        },
      ) => {
        for (oa, na) in old_attrs.iter().zip(new_attrs.iter()) {
          if oa == na {
            continue;
          }

          let name = na.name();
          let value = attr_display_value(na);

          js.push_str(&format!(
            "var e=document.querySelector(\
             '[data-zo-cmd=\"{idx}\"]');\
             if(e)e.setAttribute({},{});",
            escape_js_string(name),
            escape_js_string(&value),
          ));
        }
      }
      _ => {}
    }
  }

  js
}

/// Extract the display-string value of an `Attr`, collapsing
/// Prop / Dynamic / Style into a single `String`. Event attrs
/// flow through `UiCommand::Event` and return empty here.
fn attr_display_value(attr: &zo_ui_protocol::Attr) -> String {
  use zo_ui_protocol::Attr;

  match attr {
    Attr::Prop { value, .. } => value.to_display(),
    Attr::Dynamic { initial, .. } => initial.to_display(),
    Attr::Style { value, .. } => value.clone(),
    Attr::Event { .. } => String::new(),
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

#[cfg(test)]
mod tests {
  use super::*;

  /// Build a `zo://localhost{path}` request. The path is
  /// normalized to forward slashes with a leading `/` so
  /// Windows temp paths (which contain backslashes) produce a
  /// valid URI instead of `InvalidUriChar`.
  fn request(path: &str) -> wry::http::Request<Vec<u8>> {
    let forward = path.replace('\\', "/");
    let with_leading = if forward.starts_with('/') {
      forward
    } else {
      format!("/{forward}")
    };

    wry::http::Request::builder()
      .uri(format!("zo://localhost{with_leading}"))
      .body(Vec::new())
      .unwrap()
  }

  #[test]
  fn mime_from_path_known_extensions() {
    assert_eq!(mime_from_path("/a.html"), "text/html");
    assert_eq!(mime_from_path("/a.htm"), "text/html");
    assert_eq!(mime_from_path("/a.js"), "text/javascript");
    assert_eq!(mime_from_path("/a.css"), "text/css");
    assert_eq!(mime_from_path("/a.jpg"), "image/jpeg");
    assert_eq!(mime_from_path("/a.jpeg"), "image/jpeg");
    assert_eq!(mime_from_path("/a.png"), "image/png");
    assert_eq!(mime_from_path("/a.gif"), "image/gif");
    assert_eq!(mime_from_path("/a.webp"), "image/webp");
    assert_eq!(mime_from_path("/a.svg"), "image/svg+xml");
  }

  #[test]
  fn mime_from_path_case_insensitive() {
    assert_eq!(mime_from_path("/a.PNG"), "image/png");
    assert_eq!(mime_from_path("/a.JPG"), "image/jpeg");
  }

  #[test]
  fn mime_from_path_unknown_is_octet_stream() {
    assert_eq!(mime_from_path("/a.xyz"), "application/octet-stream");
    assert_eq!(mime_from_path("/noext"), "application/octet-stream");
  }

  #[test]
  fn serve_asset_root_returns_html_document() {
    let html = "<!DOCTYPE html><html><body>hi</body></html>";
    let response = serve_asset(html, request("/"));

    assert_eq!(response.status(), 200);
    assert_eq!(
      response
        .headers()
        .get(wry::http::header::CONTENT_TYPE)
        .unwrap(),
      "text/html",
    );
    assert_eq!(response.body().as_ref(), html.as_bytes());
  }

  #[test]
  fn serve_asset_missing_file_returns_404() {
    let response =
      serve_asset("doc", request("/definitely/does/not/exist.png"));

    assert_eq!(response.status(), 404);
  }

  #[test]
  fn serve_asset_existing_file_returns_bytes_and_mime() {
    // Write a real file so `std::fs::read` succeeds and we
    // can verify the content-type dispatch.
    let tmp = std::env::temp_dir().join("zo_serve_asset_test.png");
    let payload: &[u8] = b"\x89PNG\r\n\x1a\nfake";

    std::fs::write(&tmp, payload).unwrap();

    let path = format!("{}", tmp.to_string_lossy());
    let response = serve_asset("doc", request(&path));

    assert_eq!(response.status(), 200);
    assert_eq!(
      response
        .headers()
        .get(wry::http::header::CONTENT_TYPE)
        .unwrap(),
      "image/png",
    );
    assert_eq!(response.body().as_ref(), payload);

    let _ = std::fs::remove_file(&tmp);
  }

  // ─── build_patch_js tests ──────────────────────────────

  use zo_ui_protocol::{Attr, ElementTag, PropValue};

  fn text(s: &str) -> UiCommand {
    UiCommand::Text(s.into())
  }

  fn img(attrs: Vec<Attr>) -> UiCommand {
    UiCommand::Element {
      tag: ElementTag::Img,
      attrs,
      self_closing: true,
    }
  }

  #[test]
  fn build_patch_js_empty_when_nothing_changed() {
    let old = vec![text("hello"), text("world")];
    let new = old.clone();

    assert_eq!(build_patch_js(&old, &new), "");
  }

  #[test]
  fn build_patch_js_text_change_emits_textcontent_patch() {
    let old = vec![text("hello"), text("world")];
    let new = vec![text("hello"), text("zo")];

    let js = build_patch_js(&old, &new);

    // Only the second command changed — expect one patch on
    // idx=1 with the new content.
    assert!(
      js.contains("data-zo-cmd=\\\"1\\\"") || js.contains("data-zo-cmd=\"1\""),
      "should target index 1, got: {js}"
    );
    assert!(js.contains("textContent"), "should set textContent: {js}");
    assert!(js.contains("zo"), "should contain new value: {js}");
    assert!(
      !js.contains("hello"),
      "should NOT contain unchanged value: {js}"
    );
  }

  #[test]
  fn build_patch_js_element_attr_change_emits_setattribute() {
    let old = vec![img(vec![
      Attr::str_prop("data-id", "img_0"),
      Attr::str_prop("src", "/a.png"),
    ])];
    let new = vec![img(vec![
      Attr::str_prop("data-id", "img_0"),
      Attr::str_prop("src", "/b.png"),
    ])];

    let js = build_patch_js(&old, &new);

    assert!(js.contains("setAttribute"), "should setAttribute: {js}");
    assert!(js.contains("src"), "should target src attr: {js}");
    assert!(js.contains("/b.png"), "should use new value: {js}");
    assert!(!js.contains("/a.png"), "should NOT include old value: {js}");
  }

  #[test]
  fn build_patch_js_dynamic_attr_change_emits_setattribute() {
    let old = vec![img(vec![
      Attr::str_prop("data-id", "img_0"),
      Attr::Dynamic {
        name: "width".into(),
        var: 42,
        initial: PropValue::Num(128),
      },
    ])];
    let new = vec![img(vec![
      Attr::str_prop("data-id", "img_0"),
      Attr::Dynamic {
        name: "width".into(),
        var: 42,
        initial: PropValue::Num(256),
      },
    ])];

    let js = build_patch_js(&old, &new);

    assert!(js.contains("setAttribute"), "should setAttribute: {js}");
    assert!(js.contains("width"), "should target width attr: {js}");
    assert!(js.contains("256"), "should use new value: {js}");
  }

  #[test]
  fn build_patch_js_unchanged_attr_within_element_is_skipped() {
    // Only `src` changed; `data-id` stayed the same — expect
    // exactly one setAttribute call.
    let old = vec![img(vec![
      Attr::str_prop("data-id", "img_0"),
      Attr::str_prop("src", "/a.png"),
    ])];
    let new = vec![img(vec![
      Attr::str_prop("data-id", "img_0"),
      Attr::str_prop("src", "/b.png"),
    ])];

    let js = build_patch_js(&old, &new);

    assert_eq!(
      js.matches("setAttribute").count(),
      1,
      "exactly one setAttribute call expected, got: {js}"
    );
  }

  #[test]
  fn build_patch_js_multiple_commands_one_patch_per_change() {
    let old = vec![
      img(vec![Attr::str_prop("src", "/a.png")]),
      text("hello"),
      img(vec![Attr::str_prop("src", "/c.png")]),
    ];
    let new = vec![
      img(vec![Attr::str_prop("src", "/b.png")]),
      text("hello"),
      img(vec![Attr::str_prop("src", "/d.png")]),
    ];

    let js = build_patch_js(&old, &new);

    // Two attr changes (idx 0 and idx 2), no text change.
    assert_eq!(
      js.matches("setAttribute").count(),
      2,
      "two setAttribute calls expected, got: {js}"
    );
    assert_eq!(
      js.matches("textContent").count(),
      0,
      "no textContent calls expected, got: {js}"
    );
  }
}
