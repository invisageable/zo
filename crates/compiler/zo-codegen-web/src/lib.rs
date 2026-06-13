//! `zo-codegen-web` — the web backend.
//!
//! Transpiles a program's `#render` UI commands into a static web
//! bundle (HTML/CSS/JS). It is a codegen phase like `zo-codegen-arm`
//! and `zo-codegen-clif`, except it emits *files* (a [`WebBundle`] the
//! linker materialises into `public/`) instead of machine code.
//!
//! There is no separate "renderer": turning `UiCommand`s into HTML
//! *is* the web codegen, so it lives here. The wry webview runtime
//! reuses [`WebGen`] for its live rendering.

mod reactive;

use reactive::ReactiveJs;

use zo_codegen_backend::WebBundle;
use zo_interner::Interner;
use zo_sir::Sir;
use zo_ui_protocol::{Attr, ElementTag, PropValue, UiCommand};

use std::path::{Path, PathBuf};

/// The web code generator: lowers a program's `#render` templates to
/// HTML. Carries a reusable output buffer so the live-render patch loop
/// doesn't reallocate per frame.
pub struct WebGen {
  html_buffer: String,
  container_stack: Vec<String>,
  /// Pre-computed class attribute string from scoped stylesheets.
  /// Empty when no scoped styles are active.
  /// Space-joined scope hashes (`_zo_a3f2 _zo_b1d4`) merged into
  /// every element's `class` attribute. Kept bare (not a rendered
  /// ` class="…"` fragment) so an authored class merges into ONE
  /// attribute — a duplicate `class` attr is dropped by browsers,
  /// which silently killed scoped class selectors.
  scope_classes: String,
}

impl WebGen {
  /// A new web generator.
  pub fn new() -> Self {
    Self {
      html_buffer: String::with_capacity(4096),
      container_stack: Vec::with_capacity(16),
      scope_classes: String::new(),
    }
  }

  /// Codegen entry: transpile `sir`'s `#render` commands into a
  /// [`WebBundle`] — `index.html` plus any referenced `assets/`. A
  /// split `app.js` / `styles.css` and CSS `background-image` assets
  /// follow.
  pub fn generate(&mut self, interner: &Interner, sir: &Sir) -> WebBundle {
    let mut commands = sir.ui_commands(interner);
    let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::new();

    // Copy referenced images into `assets/` and rewrite each `<img>`
    // src to a relative `assets/<name>`, so the bundle is
    // self-contained in a browser (no `zo://localhost` host protocol).
    self.stage_img_assets(&mut commands, &mut files);

    // Emit in-page reactivity (state + handlers + binding graph) for
    // the browser. `None` for a static page — no `window.ipc` host
    // bridge, since the bundle runs with no host.
    let reactive =
      ReactiveJs::new(sir, interner).emit(&sir.bindings(interner), &commands);

    let body = self.render_body_inner(&commands);
    let html = self.document(&body, reactive.as_deref());

    files.insert(0, (PathBuf::from("index.html"), html.into_bytes()));

    WebBundle { files }
  }

  /// Copy every `<img>` pointing at a local absolute file into the
  /// bundle (`assets/<name>`) and rewrite its `src` to that relative
  /// path. Remote URLs are left untouched; missing files are skipped
  /// (the reference still points at `assets/<name>`).
  fn stage_img_assets(
    &self,
    commands: &mut [UiCommand],
    files: &mut Vec<(PathBuf, Vec<u8>)>,
  ) {
    let mut seen: Vec<String> = Vec::new();

    for cmd in commands.iter_mut() {
      let UiCommand::Element {
        tag: ElementTag::Img,
        attrs,
        ..
      } = cmd
      else {
        continue;
      };

      for attr in attrs.iter_mut() {
        let value = match attr {
          Attr::Prop { name, value } if name == "src" => value,
          Attr::Dynamic { name, initial, .. } if name == "src" => initial,
          _ => continue,
        };

        let PropValue::Str(src) = value else {
          continue;
        };

        if src.starts_with("http://") || src.starts_with("https://") {
          continue;
        }

        let path = Path::new(src.as_str());

        if !path.is_absolute() {
          continue;
        }

        let Some(name) =
          path.file_name().and_then(|n| n.to_str()).map(String::from)
        else {
          continue;
        };

        if !seen.contains(&name)
          && let Ok(bytes) = std::fs::read(path)
        {
          files.push((PathBuf::from(format!("assets/{name}")), bytes));
          seen.push(name.clone());
        }

        *src = format!("assets/{name}");
      }
    }
  }

  /// Render UI commands to a complete HTML document for the **webview**
  /// — host-side reactivity via bridge.js' IPC. (The static bundle goes
  /// through [`generate`](Self::generate), which injects in-page JS
  /// instead.) Interactive surfaces — any `<button>`, `<input>`,
  /// `<textarea>`, or `UiCommand::Event` — get the bridge injected.
  pub fn render_to_html(&mut self, commands: &[UiCommand]) -> String {
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

    let body = self.render_body_inner(commands);
    let script =
      needs_interactivity.then_some(include_str!("../assets/bridge.js"));

    self.document(&body, script)
  }

  /// Wrap a rendered `body` and an optional `<script>` into a full HTML
  /// document. The webview passes bridge.js (host IPC); the static
  /// bundle passes its in-page reactive JS.
  fn document(&self, body: &str, script: Option<&str>) -> String {
    let mut out = String::with_capacity(body.len() + 2048);

    out.push_str("<!DOCTYPE html><html><head>");
    out.push_str("<meta charset=UTF-8>");
    out.push_str(
      "<meta name=viewport content=\"width=device-width,initial-scale=1\">",
    );
    out.push_str("<title>zo</title>");
    out.push_str("<style>");
    out.push_str(include_str!("../assets/default.css"));
    out.push_str("</style>");
    out.push_str("</head><body>");
    out.push_str(body);

    if let Some(js) = script {
      out.push_str("<script>");
      out.push_str(js);
      out.push_str("</script>");
    }

    out.push_str("</body></html>");
    out
  }

  /// Render UI commands to body-inner HTML — no `<html>`, `<head>`,
  /// `<body>` wrappers, no `<script>` injection. Shared between the
  /// initial-render path (wrapped by `render_to_html`) and the live
  /// webview's per-event patch loop (used when the command buffer
  /// length changes — e.g. a list-binding splice grew it — to replace
  /// `document.body.innerHTML`). Bridge.js delegates from `document`,
  /// so handlers survive the innerHTML swap.
  pub fn render_body_inner(&mut self, commands: &[UiCommand]) -> String {
    self.html_buffer.clear();
    self.container_stack.clear();

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

    self.scope_classes = scope_hashes.join(" ");

    for (idx, cmd) in commands.iter().enumerate() {
      self.render_command(cmd, idx);
    }

    while !self.container_stack.is_empty() {
      self.end_container();
    }

    self.html_buffer.clone()
  }

  fn render_command(&mut self, cmd: &UiCommand, idx: usize) {
    match cmd {
      UiCommand::Event { .. } => {
        // Events are handled via data attributes and JS.
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
        let zo_cmd_attr = format!("data-zo-cmd=\"{idx}\"");

        let authored_class = attrs
          .iter()
          .find(|attr| attr.name() == "class")
          .and_then(|attr| attr.as_str());

        let class_attr = match (authored_class, self.scope_classes.is_empty()) {
          (Some(classes), false) => {
            format!(" class=\"{classes} {}\"", self.scope_classes)
          }
          (Some(classes), true) => format!(" class=\"{classes}\""),
          (None, false) => format!(" class=\"{}\"", self.scope_classes),
          (None, true) => String::new(),
        };

        self
          .html_buffer
          .push_str(&format!("<{tag_name}{class_attr} {zo_cmd_attr}"));

        for attr in attrs {
          // The merged attribute above already carries the class.
          if attr.name() == "class" {
            continue;
          }

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
        // Wrap text in an inline span carrying a uniform `data-zo-cmd`
        // id so reactive updates can target it via
        // `document.querySelector('[data-zo-cmd="N"]')`. Non-reactive
        // text also gets the wrapper — the cost is negligible and it
        // keeps patching uniform.
        self.html_buffer.push_str(&format!(
          "<span data-zo-cmd=\"{idx}\">{}</span>",
          escape_html(content),
        ));
      }
    }
  }

  /// Emit a single HTML attribute onto `self.html_buffer` for the given
  /// element tag. Handles per-tag rewrites (notably the `zo://localhost`
  /// src prefix for Img).
  fn emit_attr(&mut self, tag: &ElementTag, attr: &Attr) {
    match attr {
      Attr::Prop { name, value } => {
        self.emit_value_attr(tag, name, &value.to_display());
      }
      Attr::Dynamic { name, initial, .. } => {
        self.emit_value_attr(tag, name, &initial.to_display());
      }
      Attr::Style { name, value } => {
        // Inline style shorthand — emit as a style="" segment. MVP: one
        // shorthand per element; future work collapses multiple into a
        // single style attr.
        self.html_buffer.push_str(&format!(
          " style=\"{}: {}\"",
          escape_html(name),
          escape_html(value),
        ));
      }
      Attr::Event { .. } => {
        // Events flow through UiCommand::Event + the bridge.js runtime,
        // not inline HTML attributes.
      }
    }
  }

  /// Emit one HTML attribute. Boolean attributes (`checked`,
  /// `disabled`, …) are present-means-true: a truthy `value`
  /// writes the bare name, a falsy one writes nothing — never
  /// `checked="false"`, which a browser reads as checked. Every
  /// other attribute writes `name="value"`.
  fn emit_value_attr(&mut self, tag: &ElementTag, name: &str, value: &str) {
    if is_boolean_attr(name) {
      // The push and the early return guard DIFFERENT conditions —
      // the return skips the `name="value"` path for every boolean
      // attr, the push fires only for truthy ones — so they can't
      // collapse into one `&&`.
      if value != "false" {
        self.html_buffer.push_str(&format!(" {name}"));
      }

      return;
    }

    let rendered = self.rewrite_attr_value(tag, name, value);

    self
      .html_buffer
      .push_str(&format!(" {name}=\"{}\"", escape_html(&rendered)));
  }

  /// Per-tag attribute value rewrites. Currently only Img `src` needs
  /// the `zo://localhost` protocol prefix; everything else passes
  /// through unchanged.
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
      // Bare absolute filesystem paths need the custom protocol prefix
      // so wry's webview can fetch them through
      // `zo://localhost/<abs-path>`. Remote URLs and relative paths pass
      // through untouched. The path is normalized to forward slashes
      // with a leading `/` so Windows drive paths (e.g. `C:\foo.png`)
      // become valid URI paths (`zo://localhost/C:/foo.png`).
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

impl Default for WebGen {
  fn default() -> Self {
    Self::new()
  }
}

/// True for any path recognized as absolute on either host OS family.
/// Accepts three forms regardless of the running platform so zo
/// templates stay portable:
///
/// 1. Unix-style rooted paths (`/tmp/foo.png`) — absolute on Unix,
///    treated as absolute on Windows too.
/// 2. Windows drive-letter paths (`C:\foo.png`, `C:/foo.png`) —
///    absolute on Windows, recognized on Unix too so the normalization
///    logic can be unit-tested cross-platform.
/// 3. Whatever else the host OS recognizes via
///    `std::path::Path::is_absolute`.
fn is_absolute_fs_path(value: &str) -> bool {
  if value.starts_with('/') || is_windows_drive_path(value) {
    return true;
  }

  Path::new(value).is_absolute()
}

/// Match `X:\...` or `X:/...` where X is an ASCII letter.
fn is_windows_drive_path(value: &str) -> bool {
  let bytes = value.as_bytes();

  bytes.len() >= 3
    && bytes[0].is_ascii_alphabetic()
    && bytes[1] == b':'
    && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// Normalize an absolute filesystem path into a URI path component:
/// forward slashes, with a single leading `/`. Windows drive paths like
/// `C:\Users\me\cat.png` become `/C:/Users/me/cat.png`; Unix paths like
/// `/tmp/cat.png` pass through unchanged.
fn normalize_uri_path(value: &str) -> String {
  let forward = value.replace('\\', "/");

  if forward.starts_with('/') {
    forward
  } else {
    format!("/{forward}")
  }
}

/// Escape HTML special characters to prevent XSS.
fn escape_html(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}

/// HTML boolean attributes — present means true, absent means
/// false. A reactive `checked={flag}` renders as bare `checked`
/// only when the flag is truthy.
fn is_boolean_attr(name: &str) -> bool {
  matches!(
    name,
    "checked" | "disabled" | "selected" | "readonly" | "multiple"
  )
}

#[cfg(test)]
mod tests {
  use super::{WebGen, escape_html, is_absolute_fs_path, normalize_uri_path};

  use zo_ui_protocol::{Attr, ElementTag, PropValue, UiCommand};

  #[test]
  fn test_escape_html() {
    assert_eq!(escape_html("<script>"), "&lt;script&gt;");
    assert_eq!(escape_html("a & b"), "a &amp; b");
    assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
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
    let mut webgen = WebGen::new();
    let html = webgen.render_to_html(&h1_element("hello world!"));

    assert!(html.contains("hello world!"));
    assert!(html.contains("</h1>"));
    // Every element carries a uniform `data-zo-cmd` id for granular
    // reactive patching.
    assert!(html.contains("data-zo-cmd=\"0\""));
  }

  #[test]
  fn test_render_container() {
    let mut webgen = WebGen::new();
    let commands = vec![
      UiCommand::Element {
        tag: ElementTag::Div,
        attrs: vec![Attr::str_prop("data-id", "root")],
        self_closing: false,
      },
      UiCommand::Text("test".into()),
      UiCommand::EndElement,
    ];

    let html = webgen.render_to_html(&commands);

    assert!(html.contains("<div"));
    assert!(html.contains("</div>"));
    assert!(html.contains("test"));
  }

  #[test]
  fn test_xss_prevention() {
    let mut webgen = WebGen::new();
    let commands = vec![
      UiCommand::Element {
        tag: ElementTag::P,
        attrs: vec![Attr::str_prop("data-id", "p_0")],
        self_closing: false,
      },
      UiCommand::Text("<script>alert('xss')</script>".into()),
      UiCommand::EndElement,
    ];

    let html = webgen.render_to_html(&commands);

    assert!(!html.contains("<script>alert"));
    assert!(html.contains("&lt;script&gt;"));
  }

  #[test]
  fn test_render_image_absolute_path_wraps_in_zo_protocol() {
    let mut webgen = WebGen::new();
    let img = UiCommand::Element {
      tag: ElementTag::Img,
      attrs: vec![Attr::str_prop("src", "/Users/me/pictures/cat.png")],
      self_closing: true,
    };

    let html = webgen.render_to_html(&[img]);

    assert!(
      html.contains("src=\"zo://localhost/Users/me/pictures/cat.png\""),
      "absolute path should be wrapped in zo:// protocol, got: {html}"
    );
  }

  #[test]
  fn test_render_image_http_url_passes_through() {
    let mut webgen = WebGen::new();
    let img = UiCommand::Element {
      tag: ElementTag::Img,
      attrs: vec![Attr::str_prop("src", "http://example.com/a.png")],
      self_closing: true,
    };

    let html = webgen.render_to_html(&[img]);

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
  fn test_is_absolute_fs_path_cross_platform() {
    assert!(is_absolute_fs_path("/tmp/foo.png"));
    assert!(is_absolute_fs_path("/Users/me/cat.png"));
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
  }

  fn img_src(cmd: &UiCommand) -> &str {
    let UiCommand::Element { attrs, .. } = cmd else {
      return "";
    };

    for attr in attrs {
      if let Attr::Prop {
        name,
        value: PropValue::Str(src),
      } = attr
        && name == "src"
      {
        return src;
      }
    }

    ""
  }

  #[test]
  fn stage_img_assets_leaves_remote_and_relative_untouched() {
    let mut commands = vec![
      UiCommand::Element {
        tag: ElementTag::Img,
        attrs: vec![Attr::str_prop("src", "http://example.com/a.png")],
        self_closing: true,
      },
      UiCommand::Element {
        tag: ElementTag::Img,
        attrs: vec![Attr::str_prop("src", "relative/b.png")],
        self_closing: true,
      },
    ];

    let mut files = Vec::new();

    WebGen::new().stage_img_assets(&mut commands, &mut files);

    assert!(files.is_empty(), "no local absolute assets to stage");
    assert_eq!(img_src(&commands[0]), "http://example.com/a.png");
    assert_eq!(img_src(&commands[1]), "relative/b.png");
  }
}
