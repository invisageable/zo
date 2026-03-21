//! zo-ui-protocol - The shared contract between zo compiler and runtime
//!
//! This crate defines the UI command protocol that allows compiled zo code
//! to communicate with the zo runtime for rendering user interfaces.

use serde::{Deserialize, Serialize};

/// The core UI command enum - this is the entire UI language
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum UiCommand {
  /// Begin a container with layout direction
  BeginContainer {
    id: String,
    direction: ContainerDirection,
  },

  /// End the last container
  EndContainer,

  /// Draw a text element (for h1, p, span, etc.)
  Text { content: String, style: TextStyle },

  /// Draw a clickable button
  Button { id: u32, content: String },

  /// Draw a text input field
  TextInput {
    id: u32,
    placeholder: String,
    value: String,
  },

  /// Draw an image
  Image {
    id: String,
    src: String,
    width: u32,
    height: u32,
  },

  /// Event command (for event routing)
  Event {
    widget_id: String,
    event_kind: EventKind,
    handler: String,
  },
}

impl UiCommand {
  /// Get the numeric type code for memory layout (used in ARM codegen)
  pub fn type_code(&self) -> u32 {
    match self {
      Self::BeginContainer { .. } => 0,
      Self::EndContainer => 1,
      Self::Text { .. } => 2,
      Self::Button { .. } => 3,
      Self::TextInput { .. } => 4,
      Self::Image { .. } => 5,
      Self::Event { .. } => 6,
    }
  }
}

/// Container layout direction
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ContainerDirection {
  Horizontal, // for <div>, <span>
  Vertical,   // for <section>, default
}

impl ContainerDirection {
  /// Get numeric value for memory layout
  pub fn as_u32(&self) -> u32 {
    match self {
      Self::Horizontal => 0,
      Self::Vertical => 1,
    }
  }
}

/// Text styling options
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum TextStyle {
  Heading1,  // <h1>
  Heading2,  // <h2>
  Heading3,  // <h3>
  Paragraph, // <p>
  Normal,    // plain text
}

impl TextStyle {
  /// Get numeric value for memory layout
  pub fn as_u32(&self) -> u32 {
    match self {
      Self::Normal => 0,
      Self::Heading1 => 1,
      Self::Heading2 => 2,
      Self::Heading3 => 3,
      Self::Paragraph => 4,
    }
  }
}

/// Event types that can occur in the UI
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
  /// Dynamic binding: class={expr} (post-MVP)
  Dynamic { name: String },
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
    let value = match raw.parse::<u32>() {
      Ok(n) => PropValue::Num(n),
      Err(_) => match raw {
        "true" => PropValue::Bool(true),
        "false" => PropValue::Bool(false),
        _ => PropValue::Str(raw.to_string()),
      },
    };

    Self::Prop {
      name: name.to_string(),
      value,
    }
  }

  /// Get the string value of a Prop, or None.
  pub fn as_str(&self) -> Option<&str> {
    match self {
      Self::Prop {
        value: PropValue::Str(s),
        ..
      } => Some(s),
      _ => None,
    }
  }

  /// Get the numeric value of a Prop, or None.
  pub fn as_num(&self) -> Option<u32> {
    match self {
      Self::Prop {
        value: PropValue::Num(n),
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
