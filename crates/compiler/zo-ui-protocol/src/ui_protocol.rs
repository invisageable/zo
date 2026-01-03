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
    event_type: EventType,
  },
}
impl UiCommand {
  /// Get the numeric type code for memory layout (used in ARM codegen)
  pub fn type_code(&self) -> u32 {
    match self {
      UiCommand::BeginContainer { .. } => 0,
      UiCommand::EndContainer => 1,
      UiCommand::Text { .. } => 2,
      UiCommand::Button { .. } => 3,
      UiCommand::TextInput { .. } => 4,
      UiCommand::Image { .. } => 5,
      UiCommand::Event { .. } => 6,
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
      ContainerDirection::Horizontal => 0,
      ContainerDirection::Vertical => 1,
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
      TextStyle::Normal => 0,
      TextStyle::Heading1 => 1,
      TextStyle::Heading2 => 2,
      TextStyle::Heading3 => 3,
      TextStyle::Paragraph => 4,
    }
  }
}

/// Event types that can occur in the UI
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum EventType {
  Click,
  Change,
  Input,
  Focus,
  Blur,
}
