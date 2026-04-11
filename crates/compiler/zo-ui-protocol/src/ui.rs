//! A typed collection of UI commands with transformations.
//!
//! Anything that operates on a sequence of [`UiCommand`]s
//! — path resolution, validation, rewriting — belongs here,
//! not scattered across runtime drivers.

use crate::{Attr, ElementTag, PropValue, UiCommand};

use std::path::Path;

/// Owned sequence of [`UiCommand`]s with transformation
/// methods. Used as the single entry point for mutating a
/// batch of commands before handing them to a runtime.
#[derive(Debug, Default)]
pub struct Ui {
  commands: Vec<UiCommand>,
}

impl Ui {
  /// Create a [`Ui`] from an existing command vector.
  pub fn new(commands: Vec<UiCommand>) -> Self {
    Self { commands }
  }

  /// Borrow the underlying command slice.
  pub fn as_slice(&self) -> &[UiCommand] {
    &self.commands
  }

  /// Consume the [`Ui`] and return the inner command vector.
  pub fn into_commands(self) -> Vec<UiCommand> {
    self.commands
  }

  /// Number of commands in the batch.
  pub fn len(&self) -> usize {
    self.commands.len()
  }

  /// `true` when the batch is empty.
  pub fn is_empty(&self) -> bool {
    self.commands.is_empty()
  }

  /// Resolve `<img src="...">` paths to absolute filesystem
  /// paths, using `base_dir` as the root for relative paths.
  ///
  /// Rules:
  /// - `http://` / `https://` URLs pass through unchanged.
  /// - Absolute filesystem paths are canonicalized when
  ///   possible (collapses `..` segments, resolves symlinks).
  /// - Relative paths are joined against `base_dir` and then
  ///   canonicalized.
  /// - Missing files fall back to the joined (non-canonical)
  ///   path so the error surfaces at load time with a
  ///   meaningful message.
  pub fn resolve_image_paths(&mut self, base_dir: &Path) {
    for cmd in &mut self.commands {
      if let UiCommand::Element {
        tag: ElementTag::Img,
        attrs,
        ..
      } = cmd
      {
        for attr in attrs {
          let (name, value) = match attr {
            Attr::Prop { name, value } => (name, value),
            Attr::Dynamic { name, initial, .. } => (name, initial),
            _ => continue,
          };

          if name == "src"
            && let PropValue::Str(s) = value
          {
            rewrite_path_in_place(s, base_dir);
          }
        }
      }
    }
  }
}

/// Rewrite a path string in place: absolute paths and remote URLs
/// pass through, relative paths are joined against `base_dir`,
/// everything is canonicalized when possible.
fn rewrite_path_in_place(src: &mut String, base_dir: &Path) {
  if is_remote_url(src) {
    return;
  }

  let path = Path::new(src.as_str());
  let joined = if path.is_absolute() {
    path.to_path_buf()
  } else {
    base_dir.join(path)
  };

  let resolved = std::fs::canonicalize(&joined).unwrap_or(joined);

  *src = resolved.to_string_lossy().into_owned();
}

/// `true` when `src` is an `http://` or `https://` URL.
fn is_remote_url(src: &str) -> bool {
  src.starts_with("http://") || src.starts_with("https://")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn image(src: &str) -> UiCommand {
    UiCommand::Element {
      tag: ElementTag::Img,
      attrs: vec![
        Attr::str_prop("data-id", "img_0"),
        Attr::str_prop("src", src),
        Attr::parse_prop("width", "10"),
        Attr::parse_prop("height", "10"),
      ],
      self_closing: true,
    }
  }

  fn image_src(ui: &Ui, idx: usize) -> &str {
    match &ui.as_slice()[idx] {
      UiCommand::Element {
        tag: ElementTag::Img,
        attrs,
        ..
      } => attrs
        .iter()
        .find(|a| a.name() == "src")
        .and_then(|a| a.as_str())
        .expect("img element should have an src attr"),
      _ => panic!("expected Element(Img) at index {idx}"),
    }
  }

  #[test]
  fn is_remote_url_recognizes_http_and_https() {
    assert!(is_remote_url("http://example.com/a.png"));
    assert!(is_remote_url("https://example.com/a.png"));
    assert!(!is_remote_url("/tmp/a.png"));
    assert!(!is_remote_url("a.png"));
    assert!(!is_remote_url("file:///tmp/a.png"));
  }

  #[test]
  fn resolve_image_paths_passes_http_url_through() {
    let mut ui = Ui::new(vec![image("http://example.com/a.png")]);

    ui.resolve_image_paths(Path::new("/tmp"));

    assert_eq!(image_src(&ui, 0), "http://example.com/a.png");
  }

  #[test]
  fn resolve_image_paths_passes_https_url_through() {
    let mut ui = Ui::new(vec![image("https://httpbin.org/image/png")]);

    ui.resolve_image_paths(Path::new("/tmp"));

    assert_eq!(image_src(&ui, 0), "https://httpbin.org/image/png");
  }

  #[test]
  fn resolve_image_paths_joins_relative_against_base_dir() {
    // std::env::temp_dir() exists, but "missing.png" inside
    // it does not — so canonicalize falls back to the raw
    // join and we can assert on that.
    let base = std::env::temp_dir();
    let mut ui = Ui::new(vec![image("missing.png")]);

    ui.resolve_image_paths(&base);

    let expected = base.join("missing.png");

    assert_eq!(image_src(&ui, 0), expected.to_string_lossy());
  }

  #[test]
  fn resolve_image_paths_canonicalizes_existing_absolute() {
    let tmp = std::env::temp_dir().join("zo_ui_resolve_abs.png");

    std::fs::write(&tmp, b"not a real png").unwrap();

    let canonical = std::fs::canonicalize(&tmp).unwrap();

    let mut ui = Ui::new(vec![image(&tmp.to_string_lossy())]);

    ui.resolve_image_paths(Path::new("/unused"));

    assert_eq!(image_src(&ui, 0), canonical.to_string_lossy());

    let _ = std::fs::remove_file(&tmp);
  }

  #[test]
  fn resolve_image_paths_ignores_non_image_commands() {
    let mut ui = Ui::new(vec![
      UiCommand::Text("hi".into()),
      UiCommand::Element {
        tag: ElementTag::Div,
        attrs: vec![Attr::str_prop("data-id", "c")],
        self_closing: false,
      },
    ]);

    ui.resolve_image_paths(Path::new("/tmp"));

    assert!(matches!(ui.as_slice()[0], UiCommand::Text(_)));
    assert!(matches!(
      ui.as_slice()[1],
      UiCommand::Element {
        tag: ElementTag::Div,
        ..
      }
    ));
  }

  #[test]
  fn len_and_is_empty() {
    let empty = Ui::new(Vec::new());

    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);

    let one = Ui::new(vec![image("a.png")]);

    assert!(!one.is_empty());
    assert_eq!(one.len(), 1);
  }

  #[test]
  fn into_commands_returns_unchanged_vec_when_no_transform() {
    let src = vec![image("a.png"), image("b.png")];
    let ui = Ui::new(src.clone());

    assert_eq!(ui.into_commands().len(), src.len());
  }
}
