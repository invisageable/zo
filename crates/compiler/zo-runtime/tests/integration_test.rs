//! Integration test for template loading and rendering

use zo_runtime::{Graphics, Runtime, RuntimeConfig};
use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};

#[test]
fn test_runtime_with_commands() {
  // Create a runtime
  let mut runtime = Runtime::new();

  // Create test commands
  let commands = vec![
    UiCommand::BeginContainer {
      id: "test_container".to_string(),
      direction: ContainerDirection::Vertical,
    },
    UiCommand::Text {
      content: "Test Header".to_string(),
      style: TextStyle::Heading1,
    },
    UiCommand::Button {
      id: 1,
      content: "Test Button".to_string(),
    },
    UiCommand::EndContainer,
  ];

  // Set commands
  runtime.set_commands(commands.clone());

  // Verify we can create runtime without panic
  assert_eq!(commands.len(), 4);
  println!(
    "Runtime created successfully with {} commands",
    commands.len()
  );
}

#[test]
#[ignore] // Ignore by default since it requires a compiled library
fn test_runtime_library_loading() {
  // This test would require an actual compiled zo library
  let config = RuntimeConfig {
    library_path: Some("/tmp/test_template.dylib".to_string()),
    title: "Test App".to_string(),
    size: (640.0, 480.0),
    graphics: Graphics::Native,
  };

  let _runtime = Runtime::with_config(config);

  // In a real test, we would:
  // 1. Generate a test library with ARM codegen
  // 2. Load it here
  // 3. Verify commands were parsed correctly

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

  // Create simple test commands
  let commands = vec![
    UiCommand::BeginContainer {
      id: "container".to_string(),
      direction: ContainerDirection::Vertical,
    },
    UiCommand::Text {
      content: "Web Rendering Test".to_string(),
      style: TextStyle::Normal,
    },
    UiCommand::EndContainer,
  ];

  runtime.set_commands(commands);

  println!("Runtime configured for web graphics backend");
}