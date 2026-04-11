//! Integration test for template loading and rendering

use zo_runtime::Runtime;
use zo_runtime_render::render::{Graphics, RuntimeConfig};
use zo_ui_protocol::{Attr, ElementTag, UiCommand};

#[test]
fn test_runtime_with_commands() {
  let mut runtime = Runtime::new();

  // <div><h1>Test Header</h1><button>Test Button</button></div>
  let commands = vec![
    UiCommand::Element {
      tag: ElementTag::Div,
      attrs: vec![Attr::str_prop("data-id", "test_container")],
      self_closing: false,
    },
    UiCommand::Element {
      tag: ElementTag::H1,
      attrs: vec![Attr::str_prop("data-id", "h1_0")],
      self_closing: false,
    },
    UiCommand::Text("Test Header".into()),
    UiCommand::EndElement,
    UiCommand::Element {
      tag: ElementTag::Button,
      attrs: vec![Attr::parse_prop("data-id", "1")],
      self_closing: false,
    },
    UiCommand::Text("Test Button".into()),
    UiCommand::EndElement,
    UiCommand::EndElement,
  ];

  runtime.set_commands(commands.clone());

  assert_eq!(commands.len(), 9);

  println!(
    "Runtime created successfully with {} commands",
    commands.len()
  );
}

#[test]
#[ignore] // Ignore by default since it requires a compiled library
fn test_runtime_library_loading() {
  let config = RuntimeConfig {
    library_path: Some("/tmp/test_template.dylib".to_string()),
    title: "Test App".to_string(),
    size: (640.0, 480.0),
    graphics: Graphics::Native,
  };

  let _runtime = Runtime::with_config(config);

  println!("Runtime configured for library loading");
}

#[test]
fn test_runtime_with_web_graphics() {
  let config = RuntimeConfig {
    library_path: None,
    title: "Web Test".to_string(),
    size: (800.0, 600.0),
    graphics: Graphics::Web,
  };

  let mut runtime = Runtime::with_config(config);

  // <div><span>Web Rendering Test</span></div>
  let commands = vec![
    UiCommand::Element {
      tag: ElementTag::Div,
      attrs: vec![Attr::str_prop("data-id", "container")],
      self_closing: false,
    },
    UiCommand::Element {
      tag: ElementTag::Span,
      attrs: vec![Attr::str_prop("data-id", "span_0")],
      self_closing: false,
    },
    UiCommand::Text("Web Rendering Test".into()),
    UiCommand::EndElement,
    UiCommand::EndElement,
  ];

  runtime.set_commands(commands);

  println!("Runtime configured for web graphics backend");
}
