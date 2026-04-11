use zo_ui_protocol::{Attr, ElementTag, UiCommand};

use rustc_hash::FxHashMap as HashMap;

/// HTML renderer that converts UiCommands to HTML
pub struct HtmlRenderer {
  html_buffer: String,
  container_stack: Vec<String>,
  /// Maps widget_id → handler name (built from Event commands)
  event_map: HashMap<String, String>,
  /// Pre-computed class attribute string from scoped
  /// stylesheets. Empty when no scoped styles are active.
  scope_class_attr: String,
}

impl HtmlRenderer {
  /// Creates an html [`Renderer`] instance.
  pub fn new() -> Self {
    Self {
      html_buffer: String::with_capacity(4096),
      container_stack: Vec::with_capacity(16),
      event_map: HashMap::default(),
      scope_class_attr: String::new(),
    }
  }

  /// Render UI commands to complete HTML document
  pub fn render_to_html(&mut self, commands: &[UiCommand]) -> String {
    self.html_buffer.clear();
    self.container_stack.clear();

    // Detect if interactivity is needed — any `<button>`,
    // `<input>`, `<textarea>` element or any `UiCommand::Event`
    // triggers the bridge JS injection.
    let needs_interactivity = commands.iter().any(|cmd| {
      matches!(cmd, UiCommand::Event { .. })
        || matches!(
          cmd,
          UiCommand::Element {
            tag: ElementTag::Button | ElementTag::Input | ElementTag::Textarea,
            ..
          }
        )
    });

    // Minimal HTML boilerplate
    self.html_buffer.push_str("<!DOCTYPE html><html><head>");
    self.html_buffer.push_str("<meta charset=UTF-8>");
    self.html_buffer.push_str(
      "<meta name=viewport content=\"width=device-width,initial-scale=1\">",
    );
    self.html_buffer.push_str("<title>zo</title>");

    // Inline minimal CSS
    self.html_buffer.push_str("<style>");
    self
      .html_buffer
      .push_str(include_str!("../assets/default.css"));
    self.html_buffer.push_str("</style>");
    self.html_buffer.push_str("</head><body>");

    // Build scope class attribute once for all elements.
    let mut scope_hashes = Vec::new();

    for cmd in commands {
      if let UiCommand::StyleSheet {
        scope: zo_ui_protocol::StyleScope::Scoped,
        scope_hash: Some(hash),
        ..
      } = cmd
      {
        scope_hashes.push(hash.as_str());
      }
    }

    self.scope_class_attr = if scope_hashes.is_empty() {
      String::new()
    } else {
      format!(" class=\"{}\"", scope_hashes.join(" "))
    };

    // Build widget_id → handler map from Event commands
    self.event_map.clear();
    for cmd in commands {
      if let UiCommand::Event {
        widget_id, handler, ..
      } = cmd
      {
        self.event_map.insert(widget_id.clone(), handler.clone());
      }
    }

    // Render commands with stable IDs for granular updates.
    for (idx, cmd) in commands.iter().enumerate() {
      self.render_command(cmd, idx);
    }

    // Close any remaining containers
    while !self.container_stack.is_empty() {
      self.end_container();
    }

    // Only add bridge if interactive elements present
    if needs_interactivity {
      self.html_buffer.push_str("<script>");
      self
        .html_buffer
        .push_str(include_str!("../assets/bridge.js"));
      self.html_buffer.push_str("</script>");
    }

    self.html_buffer.push_str("</body></html>");
    self.html_buffer.clone()
  }

  fn render_command(&mut self, cmd: &UiCommand, idx: usize) {
    match cmd {
      UiCommand::Event { .. } => {
        // Events are handled via data attributes and JS
      }

      UiCommand::StyleSheet { css, scope, .. } => {
        use zo_ui_protocol::StyleScope;

        let scope_attr = match scope {
          StyleScope::Scoped => " data-zo-scoped",
          StyleScope::Global => "",
        };

        self
          .html_buffer
          .push_str(&format!("<style{scope_attr}>\n{css}</style>\n"));
      }

      UiCommand::Element {
        tag,
        attrs,
        self_closing,
      } => {
        let tag_name = tag.as_str();
        let sc = &self.scope_class_attr;
        let zo_cmd_attr = format!("data-zo-cmd=\"{idx}\"");

        self
          .html_buffer
          .push_str(&format!("<{tag_name}{sc} {zo_cmd_attr}"));

        for attr in attrs {
          self.emit_attr(tag, attr);
        }

        if *self_closing {
          self.html_buffer.push_str(" />\n");
        } else {
          self.html_buffer.push('>');
          self.container_stack.push(tag_name.to_string());
        }
      }

      UiCommand::EndElement => {
        if let Some(tag_name) = self.container_stack.pop() {
          self.html_buffer.push_str(&format!("</{tag_name}>\n"));
        }
      }

      UiCommand::Text(content) => {
        // Wrap text in an inline span carrying a uniform
        // `data-zo-cmd` id so reactive updates can target it
        // via `document.querySelector('[data-zo-cmd="N"]')`.
        // Non-reactive text also gets the wrapper — the cost
        // is negligible and it keeps patching uniform.
        self.html_buffer.push_str(&format!(
          "<span data-zo-cmd=\"{idx}\">{}</span>",
          escape_html(content),
        ));
      }
    }
  }

  /// Emit a single HTML attribute onto `self.html_buffer` for the
  /// given element tag. Handles per-tag rewrites (notably the
  /// `zo://localhost` src prefix for Img).
  fn emit_attr(&mut self, tag: &ElementTag, attr: &Attr) {
    match attr {
      Attr::Prop { name, value } => {
        let s = value.to_display();
        let rendered = self.rewrite_attr_value(tag, name, &s);

        self
          .html_buffer
          .push_str(&format!(" {name}=\"{}\"", escape_html(&rendered),));
      }
      Attr::Dynamic { name, initial, .. } => {
        let s = initial.to_display();
        let rendered = self.rewrite_attr_value(tag, name, &s);

        self
          .html_buffer
          .push_str(&format!(" {name}=\"{}\"", escape_html(&rendered),));
      }
      Attr::Style { name, value } => {
        // Inline style shorthand — emit as a style=""
        // segment. MVP: one shorthand per element; future
        // work collapses multiple into a single style attr.
        self.html_buffer.push_str(&format!(
          " style=\"{}: {}\"",
          escape_html(name),
          escape_html(value),
        ));
      }
      Attr::Event { .. } => {
        // Events flow through UiCommand::Event + the bridge.js
        // runtime, not inline HTML attributes.
      }
    }
  }

  /// Per-tag attribute value rewrites. Currently only Img `src`
  /// needs the `zo://localhost` protocol prefix; everything else
  /// passes through unchanged.
  fn rewrite_attr_value(
    &self,
    tag: &ElementTag,
    name: &str,
    value: &str,
  ) -> String {
    if matches!(tag, ElementTag::Img)
      && name == "src"
      && is_absolute_fs_path(value)
    {
      // Bare absolute filesystem paths need the custom protocol
      // prefix so wry's webview can fetch them through
      // `zo://localhost/<abs-path>`. Remote URLs and relative
      // paths pass through untouched. The path is normalized to
      // forward slashes with a leading `/` so Windows drive
      // paths (e.g. `C:\foo.png`) become valid URI paths
      // (`zo://localhost/C:/foo.png`).
      return format!("zo://localhost{}", normalize_uri_path(value));
    }

    value.to_string()
  }

  fn end_container(&mut self) {
    if !self.container_stack.is_empty() {
      self.container_stack.pop();
      self.html_buffer.push_str("</div>\n");
    }
  }
}

impl Default for HtmlRenderer {
  fn default() -> Self {
    Self::new()
  }
}

/// True for any path recognized as absolute on either host OS
/// family. Accepts three forms regardless of the running
/// platform so zo templates stay portable:
///
/// 1. Unix-style rooted paths (`/tmp/foo.png`) — absolute on
///    Unix, treated as absolute on Windows too.
/// 2. Windows drive-letter paths (`C:\foo.png`, `C:/foo.png`)
///    — absolute on Windows, recognized on Unix too so the
///    normalization logic can be unit-tested cross-platform.
/// 3. Whatever else the host OS recognizes via
///    `std::path::Path::is_absolute`.
fn is_absolute_fs_path(value: &str) -> bool {
  if value.starts_with('/') || is_windows_drive_path(value) {
    return true;
  }

  std::path::Path::new(value).is_absolute()
}

/// Match `X:\...` or `X:/...` where X is an ASCII letter.
fn is_windows_drive_path(value: &str) -> bool {
  let bytes = value.as_bytes();

  bytes.len() >= 3
    && bytes[0].is_ascii_alphabetic()
    && bytes[1] == b':'
    && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// Normalize an absolute filesystem path into a URI path
/// component: forward slashes, with a single leading `/`.
/// Windows drive paths like `C:\Users\me\cat.png` become
/// `/C:/Users/me/cat.png`; Unix paths like `/tmp/cat.png` pass
/// through unchanged.
fn normalize_uri_path(value: &str) -> String {
  let forward = value.replace('\\', "/");

  if forward.starts_with('/') {
    forward
  } else {
    format!("/{forward}")
  }
}

/// Escape HTML special characters to prevent XSS
fn escape_html(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_escape_html() {
    assert_eq!(escape_html("<script>"), "&lt;script&gt;");
    assert_eq!(escape_html("a & b"), "a &amp; b");
    assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
  }

  fn p_element(text: &str) -> Vec<UiCommand> {
    vec![
      UiCommand::Element {
        tag: ElementTag::P,
        attrs: vec![Attr::str_prop("data-id", "p_0")],
        self_closing: false,
      },
      UiCommand::Text(text.into()),
      UiCommand::EndElement,
    ]
  }

  fn h1_element(text: &str) -> Vec<UiCommand> {
    vec![
      UiCommand::Element {
        tag: ElementTag::H1,
        attrs: vec![Attr::str_prop("data-id", "h1_0")],
        self_closing: false,
      },
      UiCommand::Text(text.into()),
      UiCommand::EndElement,
    ]
  }

  #[test]
  fn test_render_text() {
    let mut renderer = HtmlRenderer::new();
    let html = renderer.render_to_html(&h1_element("hello world!"));

    assert!(html.contains("hello world!"));
    assert!(html.contains("</h1>"));
    // Every element carries a uniform `data-zo-cmd` id for
    // granular reactive patching.
    assert!(html.contains("data-zo-cmd=\"0\""));
  }

  #[test]
  fn test_render_container() {
    let mut renderer = HtmlRenderer::new();
    let commands = vec![
      UiCommand::Element {
        tag: ElementTag::Div,
        attrs: vec![Attr::str_prop("data-id", "root")],
        self_closing: false,
      },
      UiCommand::Text("test".into()),
      UiCommand::EndElement,
    ];

    let html = renderer.render_to_html(&commands);

    assert!(html.contains("<div"));
    assert!(html.contains("</div>"));
    assert!(html.contains("test"));
  }

  #[test]
  fn test_xss_prevention() {
    let mut renderer = HtmlRenderer::new();
    let html =
      renderer.render_to_html(&p_element("<script>alert('xss')</script>"));

    assert!(!html.contains("<script>alert"));
    assert!(html.contains("&lt;script&gt;"));
  }

  #[test]
  fn test_scoped_style_adds_class_to_elements() {
    use zo_ui_protocol::StyleScope;

    let mut renderer = HtmlRenderer::new();
    let mut commands = vec![UiCommand::StyleSheet {
      css: "p._zo_test { color: cyan; }\n".into(),
      scope: StyleScope::Scoped,
      scope_hash: Some("_zo_test".into()),
    }];

    commands.extend(p_element("styled"));

    let html = renderer.render_to_html(&commands);

    assert!(
      html.contains("class=\"_zo_test\"") && html.contains("styled"),
      "scoped style should add class to <p>, got: {html}"
    );
    assert!(
      html.contains("<style data-zo-scoped>"),
      "should inject scoped style tag, got: {html}"
    );
    assert!(
      html.contains("color: cyan;"),
      "CSS content should be present, got: {html}"
    );
  }

  fn image_cmd(src: &str) -> UiCommand {
    UiCommand::Element {
      tag: ElementTag::Img,
      attrs: vec![
        Attr::str_prop("data-id", "img_0"),
        Attr::str_prop("src", src),
        Attr::parse_prop("width", "256"),
        Attr::parse_prop("height", "128"),
      ],
      self_closing: true,
    }
  }

  #[test]
  fn test_render_image_absolute_path_wraps_in_zo_protocol() {
    let mut renderer = HtmlRenderer::new();
    let html =
      renderer.render_to_html(&[image_cmd("/Users/me/pictures/cat.png")]);

    assert!(
      html.contains("src=\"zo://localhost/Users/me/pictures/cat.png\""),
      "absolute path should be wrapped in zo:// protocol, got: {html}"
    );
  }

  #[test]
  fn test_render_image_windows_drive_path_normalized() {
    let mut renderer = HtmlRenderer::new();
    let html = renderer.render_to_html(&[image_cmd("C:\\Users\\me\\cat.png")]);

    // Windows paths should be normalized to forward slashes
    // and prefixed with `/` so the URI is well-formed regardless
    // of the host platform.
    assert!(
      html.contains("src=\"zo://localhost/C:/Users/me/cat.png\""),
      "Windows drive path should be normalized, got: {html}"
    );
  }

  #[test]
  fn test_is_absolute_fs_path_cross_platform() {
    // Unix-style roots are absolute on every platform so
    // templates can use `/` paths portably.
    assert!(is_absolute_fs_path("/tmp/foo.png"));
    assert!(is_absolute_fs_path("/Users/me/cat.png"));
    // Relative paths are not.
    assert!(!is_absolute_fs_path("foo.png"));
    assert!(!is_absolute_fs_path("./foo.png"));
    assert!(!is_absolute_fs_path("assets/foo.png"));
  }

  #[test]
  fn test_normalize_uri_path_forward_slashes() {
    assert_eq!(normalize_uri_path("/tmp/foo.png"), "/tmp/foo.png");
    assert_eq!(
      normalize_uri_path("C:\\Users\\me\\cat.png"),
      "/C:/Users/me/cat.png"
    );
    assert_eq!(
      normalize_uri_path("C:/Users/me/cat.png"),
      "/C:/Users/me/cat.png"
    );
  }

  #[test]
  fn test_render_image_http_url_passes_through() {
    let mut renderer = HtmlRenderer::new();
    let html =
      renderer.render_to_html(&[image_cmd("http://example.com/a.png")]);

    assert!(
      html.contains("src=\"http://example.com/a.png\""),
      "http URL should pass through unchanged, got: {html}"
    );
    assert!(
      !html.contains("zo://localhost"),
      "http URL must not be wrapped in zo://, got: {html}"
    );
  }

  #[test]
  fn test_render_image_https_url_passes_through() {
    let mut renderer = HtmlRenderer::new();
    let html =
      renderer.render_to_html(&[image_cmd("https://httpbin.org/image/png")]);

    assert!(
      html.contains("src=\"https://httpbin.org/image/png\""),
      "https URL should pass through unchanged, got: {html}"
    );
  }

  #[test]
  fn test_render_image_preserves_dimensions() {
    let mut renderer = HtmlRenderer::new();
    let html = renderer.render_to_html(&[image_cmd("/tmp/x.png")]);

    assert!(html.contains("width=\"256\""));
    assert!(html.contains("height=\"128\""));
  }

  #[test]
  fn test_global_style_no_class_on_elements() {
    use zo_ui_protocol::StyleScope;

    let mut renderer = HtmlRenderer::new();
    let mut commands = vec![UiCommand::StyleSheet {
      css: "body { margin: 0; }\n".into(),
      scope: StyleScope::Global,
      scope_hash: None,
    }];

    commands.extend(p_element("plain"));

    let html = renderer.render_to_html(&commands);

    // Global style: no scope class attribute on elements.
    assert!(
      html.contains("plain") && !html.contains(" class="),
      "global style should NOT add a scope class, got: {html}"
    );
    // Style tag should not have scoped attribute.
    assert!(
      html.contains("<style>\n"),
      "global style tag should not be scoped, got: {html}"
    );
  }
}
