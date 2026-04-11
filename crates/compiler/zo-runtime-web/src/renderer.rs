use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};

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

    // Detect if interactivity is needed
    let needs_interactivity = commands.iter().any(|cmd| {
      matches!(
        cmd,
        UiCommand::Button { .. }
          | UiCommand::TextInput { .. }
          | UiCommand::Event { .. }
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
      UiCommand::BeginContainer { id, direction } => {
        let layout = match direction {
          ContainerDirection::Horizontal => "container-horizontal",
          ContainerDirection::Vertical => "container-vertical",
        };

        // Append scope classes if present.
        let sc = &self.scope_class_attr;
        let class_attr = if sc.is_empty() {
          format!("class=\"{layout}\"")
        } else {
          // sc already starts with ` class="..."`, extract
          // the hashes and merge with layout class.
          let hashes = sc.trim_start_matches(" class=\"").trim_end_matches('"');

          format!("class=\"{layout} {hashes}\"")
        };

        self.html_buffer.push_str(&format!(
          "<div {class_attr} data-id=\"{}\">\n",
          escape_html(id)
        ));

        self.container_stack.push(id.clone());
      }

      UiCommand::EndContainer => self.end_container(),

      UiCommand::Text { content, style } => {
        let tag = match style {
          TextStyle::Heading1 => "h1",
          TextStyle::Heading2 => "h2",
          TextStyle::Heading3 => "h3",
          TextStyle::Paragraph => "p",
          TextStyle::Normal => "span",
        };

        let sc = &self.scope_class_attr;

        self.html_buffer.push_str(&format!(
          "<{tag}{sc} id=\"zo-cmd-{idx}\">{}</{tag}>\n",
          escape_html(content),
        ));
      }

      UiCommand::Button { id, content } => {
        let sc = &self.scope_class_attr;

        self.html_buffer.push_str(&format!(
          "<button{sc} data-id=\"{id}\">{}</button>\n",
          escape_html(content)
        ));
      }

      UiCommand::TextInput {
        id,
        placeholder,
        value,
      } => {
        let sc = &self.scope_class_attr;

        self.html_buffer.push_str(&format!(
          "<input type=\"text\"{sc} data-id=\"{id}\" \
           placeholder=\"{}\" value=\"{}\" />\n",
          escape_html(placeholder),
          escape_html(value),
        ));
      }

      UiCommand::Image {
        id,
        src,
        width,
        height,
      } => {
        let sc = &self.scope_class_attr;

        // The webview loads via the `zo://localhost` custom
        // protocol (see zo-runtime-web/src/runtime.rs). Image
        // assets are served through the same protocol — the
        // handler strips the leading `/` and reads the file
        // from disk. Absolute paths map to `zo://localhost`
        // + the absolute path; relative paths pass through.
        let url_src = if std::path::Path::new(src.as_str()).is_absolute() {
          format!("zo://localhost{src}")
        } else {
          src.to_string()
        };

        self.html_buffer.push_str(&format!(
          "<img{sc} data-id=\"{}\" src=\"{}\" width=\"{width}\" height=\"{height}\" />\n",
          escape_html(id),
          escape_html(&url_src),
        ));
      }

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
    }
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

  #[test]
  fn test_render_text() {
    let mut renderer = HtmlRenderer::new();
    let commands = vec![UiCommand::Text {
      content: "hello world!".to_string(),
      style: TextStyle::Heading1,
    }];

    let html = renderer.render_to_html(&commands);
    assert!(html.contains("hello world!</h1>"));
    assert!(html.contains("id=\"zo-cmd-0\""));
  }

  #[test]
  fn test_render_container() {
    let mut renderer = HtmlRenderer::new();
    let commands = vec![
      UiCommand::BeginContainer {
        id: "root".to_string(),
        direction: ContainerDirection::Vertical,
      },
      UiCommand::Text {
        content: "test".to_string(),
        style: TextStyle::Normal,
      },
      UiCommand::EndContainer,
    ];

    let html = renderer.render_to_html(&commands);
    assert!(html.contains("<div class=\"container-vertical\""));
    assert!(html.contains("</div>"));
  }

  #[test]
  fn test_xss_prevention() {
    let mut renderer = HtmlRenderer::new();
    let commands = vec![UiCommand::Text {
      content: "<script>alert('xss')</script>".to_string(),
      style: TextStyle::Normal,
    }];

    let html = renderer.render_to_html(&commands);
    assert!(!html.contains("<script>alert"));
    assert!(html.contains("&lt;script&gt;"));
  }

  #[test]
  fn test_scoped_style_adds_class_to_elements() {
    use zo_ui_protocol::StyleScope;

    let mut renderer = HtmlRenderer::new();
    let commands = vec![
      UiCommand::StyleSheet {
        css: "p._zo_test { color: cyan; }\n".into(),
        scope: StyleScope::Scoped,
        scope_hash: Some("_zo_test".into()),
      },
      UiCommand::Text {
        content: "styled".into(),
        style: TextStyle::Paragraph,
      },
    ];

    let html = renderer.render_to_html(&commands);

    // The <p> should have the scope class.
    assert!(
      html.contains("class=\"_zo_test\"") && html.contains(">styled</p>"),
      "scoped style should add class to <p>, got: {html}"
    );
    // The <style> tag should be present.
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
    UiCommand::Image {
      id: "img_0".into(),
      src: src.into(),
      width: 256,
      height: 128,
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
    let commands = vec![
      UiCommand::StyleSheet {
        css: "body { margin: 0; }\n".into(),
        scope: StyleScope::Global,
        scope_hash: None,
      },
      UiCommand::Text {
        content: "plain".into(),
        style: TextStyle::Paragraph,
      },
    ];

    let html = renderer.render_to_html(&commands);

    // Global style: no class attribute on elements.
    assert!(
      html.contains(">plain</p>") && !html.contains("class="),
      "global style should NOT add class, got: {html}"
    );
    // Style tag should not have scoped attribute.
    assert!(
      html.contains("<style>\n"),
      "global style tag should not be scoped, got: {html}"
    );
  }
}
