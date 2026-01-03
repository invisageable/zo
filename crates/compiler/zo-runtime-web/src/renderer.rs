use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};

/// HTML renderer that converts UiCommands to HTML
pub struct HtmlRenderer {
  html_buffer: String,
  container_stack: Vec<String>,
}

impl HtmlRenderer {
  pub fn new() -> Self {
    Self {
      html_buffer: String::with_capacity(4096),
      container_stack: Vec::with_capacity(16),
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

    // Render commands
    for cmd in commands {
      self.render_command(cmd);
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

  fn render_command(&mut self, cmd: &UiCommand) {
    match cmd {
      UiCommand::BeginContainer { id, direction } => {
        let class = match direction {
          ContainerDirection::Horizontal => "container-horizontal",
          ContainerDirection::Vertical => "container-vertical",
        };

        self.html_buffer.push_str(&format!(
          "<div class=\"{}\" data-id=\"{}\">\n",
          class,
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

        self
          .html_buffer
          .push_str(&format!("<{tag}>{}</{tag}>\n", escape_html(content),));
      }

      UiCommand::Button { id, content } => self.html_buffer.push_str(&format!(
        "<button data-id=\"{id}\" onclick=\"handleClick({id})\">{}</button>\n",
        escape_html(content)
      )),

      UiCommand::TextInput {
        id,
        placeholder,
        value,
      } => {
        self.html_buffer.push_str(&format!(
          "<input type=\"text\" data-id=\"{id}\" placeholder=\"{}\" value=\"{}\" oninput=\"handleInput({id}, this.value)\" />\n",
          escape_html(placeholder), escape_html(value)
        ));
      }

      UiCommand::Image {
        id,
        src,
        width,
        height,
      } => {
        self.html_buffer.push_str(&format!(
          "<img data-id=\"{}\" src=\"{}\" width=\"{width}\" height=\"{height}\" />\n",
          escape_html(id),
          escape_html(src),
        ));
      }

      UiCommand::Event { .. } => {
        // Events are handled via data attributes and JS
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
    assert!(html.contains("<h1>hello world!</h1>"));
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
}
