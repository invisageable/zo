//! zo-ui-protocol - The shared contract between zo compiler and runtime
//!
//! This crate defines the UI command protocol that allows compiled zo code
//! to communicate with the zo runtime for rendering user interfaces.

use serde::{Deserialize, Serialize};

/// Stylesheet scope.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum StyleScope {
  /// `$: { ... }` — styles only apply to the component.
  Scoped,
  /// `pub $: { ... }` — styles apply globally.
  Global,
}

/// The core UI command enum — the entire UI language,
/// modeled as a stream of HTML-parity open/close events with
/// every attribute flowing through a generic `Vec<Attr>`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum UiCommand {
  /// Open an HTML-parity element. Self-closing elements (img,
  /// input, br) emit no matching `EndElement`.
  Element {
    tag: ElementTag,
    attrs: Vec<Attr>,
    self_closing: bool,
  },

  /// Close the most recently opened non-self-closing element.
  EndElement,

  /// Inline text (HTML PCDATA). No style discriminator — the
  /// enclosing element's tag carries all visual semantics.
  Text(String),

  /// Event command (for event routing).
  Event {
    widget_id: String,
    event_kind: EventKind,
    handler: String,
  },

  /// Inject a stylesheet (scoped or global).
  StyleSheet {
    css: String,
    scope: StyleScope,
    /// Scope hash for class rewriting (e.g. `"_zo_a3f2"`).
    /// Present only when `scope == Scoped`.
    scope_hash: Option<String>,
  },
}

impl UiCommand {
  /// Get the numeric type code for memory layout (used in ARM codegen).
  pub fn type_code(&self) -> u32 {
    match self {
      Self::Element { .. } => 0,
      Self::EndElement => 1,
      Self::Text(_) => 2,
      Self::Event { .. } => 3,
      Self::StyleSheet { .. } => 4,
    }
  }

  /// Generic attribute setter for reactive updates. Finds an
  /// attribute named `name` on an `Element` command and
  /// overwrites its value with the parsed form of `value`.
  /// Updates both `Attr::Prop` and `Attr::Dynamic` entries so
  /// reactive bindings stay in sync with the initial eager
  /// value. No-op on non-`Element` commands or when the
  /// attribute is absent.
  pub fn set_attr(&mut self, name: &str, value: &str) {
    let Self::Element { attrs, .. } = self else {
      return;
    };

    for attr in attrs {
      if attr.name() != name {
        continue;
      }

      match attr {
        Attr::Prop { value: v, .. } => {
          *v = PropValue::parse(value);
        }
        Attr::Dynamic { initial, .. } => {
          *initial = PropValue::parse(value);
        }
        _ => {}
      }

      return;
    }
  }
}

/// HTML-parity element tag. One variant per enumerated tag, plus
/// a `Custom(String)` escape hatch for any tag the compiler hasn't
/// wired into the classifier.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ElementTag {
  // Block containers
  Div,
  Section,
  Main,
  Article,
  Aside,
  Header,
  Footer,
  Nav,
  Form,
  Ul,
  Ol,
  Li,
  // Inline container
  Span,
  // Text (headings + paragraph)
  H1,
  H2,
  H3,
  H4,
  H5,
  H6,
  P,
  // Media
  Img,
  // Interactive
  Button,
  Input,
  Textarea,
  // Escape hatch for any HTML tag the compiler hasn't enumerated.
  Custom(String),
}

impl ElementTag {
  /// Canonical HTML tag name.
  pub fn as_str(&self) -> &str {
    match self {
      Self::Div => "div",
      Self::Section => "section",
      Self::Main => "main",
      Self::Article => "article",
      Self::Aside => "aside",
      Self::Header => "header",
      Self::Footer => "footer",
      Self::Nav => "nav",
      Self::Form => "form",
      Self::Ul => "ul",
      Self::Ol => "ol",
      Self::Li => "li",
      Self::Span => "span",
      Self::H1 => "h1",
      Self::H2 => "h2",
      Self::H3 => "h3",
      Self::H4 => "h4",
      Self::H5 => "h5",
      Self::H6 => "h6",
      Self::P => "p",
      Self::Img => "img",
      Self::Button => "button",
      Self::Input => "input",
      Self::Textarea => "textarea",
      Self::Custom(s) => s.as_str(),
    }
  }

  /// Self-closing by default in HTML5 (img, input).
  pub fn is_self_closing_default(&self) -> bool {
    matches!(self, Self::Img | Self::Input)
  }

  /// Inline vs block layout. Drives egui horizontal/vertical
  /// container selection on the native renderer.
  pub fn is_inline(&self) -> bool {
    matches!(self, Self::Span)
  }

  /// Text tags that expect inline PCDATA children and imply a
  /// specific font/weight on native. The web renderer renders them
  /// as-is; native renderers use this to pick a font size.
  pub fn is_text_tag(&self) -> bool {
    matches!(
      self,
      Self::H1
        | Self::H2
        | Self::H3
        | Self::H4
        | Self::H5
        | Self::H6
        | Self::P
        | Self::Span
    )
  }

  /// Numeric tag code for binary layout (R1.2 encoder). Codes
  /// follow the enum declaration order so headings stay grouped.
  pub fn as_u32(&self) -> u32 {
    match self {
      Self::Div => 0,
      Self::Section => 1,
      Self::Main => 2,
      Self::Article => 3,
      Self::Aside => 4,
      Self::Header => 5,
      Self::Footer => 6,
      Self::Nav => 7,
      Self::Form => 8,
      Self::Ul => 9,
      Self::Ol => 10,
      Self::Li => 11,
      Self::Span => 12,
      Self::H1 => 13,
      Self::H2 => 14,
      Self::H3 => 15,
      Self::H4 => 16,
      Self::H5 => 17,
      Self::H6 => 18,
      Self::P => 19,
      Self::Img => 20,
      Self::Button => 21,
      Self::Input => 22,
      Self::Textarea => 23,
      Self::Custom(_) => 24,
    }
  }

  /// Inverse of `as_u32`. `Custom(_)` decoding requires a separate
  /// string fetch from the binary string table.
  pub fn from_u32(code: u32) -> Option<Self> {
    Some(match code {
      0 => Self::Div,
      1 => Self::Section,
      2 => Self::Main,
      3 => Self::Article,
      4 => Self::Aside,
      5 => Self::Header,
      6 => Self::Footer,
      7 => Self::Nav,
      8 => Self::Form,
      9 => Self::Ul,
      10 => Self::Ol,
      11 => Self::Li,
      12 => Self::Span,
      13 => Self::H1,
      14 => Self::H2,
      15 => Self::H3,
      16 => Self::H4,
      17 => Self::H5,
      18 => Self::H6,
      19 => Self::P,
      20 => Self::Img,
      21 => Self::Button,
      22 => Self::Input,
      23 => Self::Textarea,
      _ => return None,
    })
  }
}

/// Event types that can occur in the UI.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum EventKind {
  Click,
  Hover,
  Change,
  Input,
  Focus,
  Blur,
}

/// Typed property value for template attributes.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum PropValue {
  /// String value: src="logo.png", placeholder="Enter name"
  Str(String),
  /// Numeric value: width="128", height="64"
  Num(u32),
  /// Boolean value: disabled, checked, readonly
  Bool(bool),
}

impl PropValue {
  /// Parse a string into the narrowest matching PropValue. Numbers
  /// beat bools beat strings. Used by attribute construction and
  /// `UiCommand::set_attr` (R3).
  pub fn parse(raw: &str) -> Self {
    if let Ok(n) = raw.parse::<u32>() {
      return Self::Num(n);
    }

    match raw {
      "true" => Self::Bool(true),
      "false" => Self::Bool(false),
      _ => Self::Str(raw.to_string()),
    }
  }

  /// Render as a display string. Mirrors the implicit conversion
  /// that happens when attributes reach the renderer.
  pub fn to_display(&self) -> String {
    match self {
      Self::Str(s) => s.clone(),
      Self::Num(n) => n.to_string(),
      Self::Bool(b) => b.to_string(),
    }
  }
}

/// Typed template attribute — compile-time representation
/// of what appears between `<tag` and `>`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Attr {
  /// Static property: src="logo.png", width="128"
  Prop { name: String, value: PropValue },
  /// Event binding: @click={handler}
  Event {
    name: String,
    event_kind: EventKind,
    handler: String,
  },
  /// Inline style: style:color="red" (post-MVP)
  Style { name: String, value: String },
  /// Reactive binding: attribute reads a mutable variable. Eagerly
  /// stringified to `initial` at compile time AND tracked so the
  /// runtime can re-patch the target command when `var` changes.
  ///
  /// `var` is a raw `u32` (interner symbol id) so `zo-ui-protocol`
  /// stays independent of `zo-interner`. Callers convert
  /// `Symbol::0` ↔ `u32` at the boundary.
  Dynamic {
    name: String,
    var: u32,
    initial: PropValue,
  },
}

impl Attr {
  /// Create a string property.
  pub fn str_prop(name: &str, value: &str) -> Self {
    Self::Prop {
      name: name.to_string(),
      value: PropValue::Str(value.to_string()),
    }
  }

  /// Create a numeric property from a string, falling back
  /// to string if parsing fails.
  pub fn parse_prop(name: &str, raw: &str) -> Self {
    Self::Prop {
      name: name.to_string(),
      value: PropValue::parse(raw),
    }
  }

  /// Get the string value of a Prop or Dynamic, or None.
  pub fn as_str(&self) -> Option<&str> {
    match self {
      Self::Prop {
        value: PropValue::Str(s),
        ..
      }
      | Self::Dynamic {
        initial: PropValue::Str(s),
        ..
      } => Some(s),
      _ => None,
    }
  }

  /// Get the numeric value of a Prop or Dynamic, or None.
  pub fn as_num(&self) -> Option<u32> {
    match self {
      Self::Prop {
        value: PropValue::Num(n),
        ..
      }
      | Self::Dynamic {
        initial: PropValue::Num(n),
        ..
      } => Some(*n),
      _ => None,
    }
  }

  /// Get the attribute name.
  pub fn name(&self) -> &str {
    match self {
      Self::Prop { name, .. } => name,
      Self::Event { name, .. } => name,
      Self::Style { name, .. } => name,
      Self::Dynamic { name, .. } => name,
    }
  }
}

#[cfg(test)]
mod set_attr_tests {
  use super::*;

  fn img_with(attrs: Vec<Attr>) -> UiCommand {
    UiCommand::Element {
      tag: ElementTag::Img,
      attrs,
      self_closing: true,
    }
  }

  #[test]
  fn set_attr_updates_prop_value() {
    let mut cmd = img_with(vec![Attr::str_prop("src", "/a.png")]);

    cmd.set_attr("src", "/b.png");

    if let UiCommand::Element { attrs, .. } = cmd {
      assert_eq!(
        attrs[0].as_str(),
        Some("/b.png"),
        "src should be updated in place"
      );
    } else {
      panic!("expected Element");
    }
  }

  #[test]
  fn set_attr_updates_dynamic_initial_value() {
    let mut cmd = img_with(vec![Attr::Dynamic {
      name: "src".into(),
      var: 7,
      initial: PropValue::Str("/a.png".into()),
    }]);

    cmd.set_attr("src", "/b.png");

    if let UiCommand::Element { attrs, .. } = cmd
      && let Attr::Dynamic { initial, .. } = &attrs[0]
    {
      assert_eq!(
        initial.to_display(),
        "/b.png",
        "dynamic initial should be updated"
      );
    } else {
      panic!("expected Element with Dynamic attr");
    }
  }

  #[test]
  fn set_attr_parses_numeric_value() {
    let mut cmd = img_with(vec![Attr::parse_prop("width", "10")]);

    cmd.set_attr("width", "128");

    if let UiCommand::Element { attrs, .. } = cmd {
      assert_eq!(attrs[0].as_num(), Some(128));
    } else {
      panic!("expected Element");
    }
  }

  #[test]
  fn set_attr_unknown_attr_is_noop() {
    let original = img_with(vec![Attr::str_prop("src", "/a.png")]);
    let mut cmd = original.clone();

    cmd.set_attr("bogus", "value");

    assert_eq!(cmd, original, "unknown attr should be a no-op");
  }

  #[test]
  fn set_attr_on_non_element_is_noop() {
    let mut cmd = UiCommand::Text("hello".to_string());

    cmd.set_attr("anything", "value");

    assert_eq!(
      cmd,
      UiCommand::Text("hello".to_string()),
      "non-element commands should be unaffected by set_attr"
    );
  }
}
