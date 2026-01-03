//! Integration test for template code generation and runtime compatibility

use zo_codegen_arm::ARM64Gen;
use zo_interner::Interner;
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};
use zo_value::ValueId;

use std::fs;

/// Test that generates a complete template binary that should be loadable by
/// zo-runtime
#[test]
fn test_complete_template_binary() {
  let mut interner = Interner::new();
  let mut sir = Sir::new();

  // Create a realistic template like one from zo source
  let template_id = ValueId(1);
  let ty_id = TyId(0);

  // Build UI commands for: <h1>Hello World</h1>
  let commands = vec![
    UiCommand::BeginContainer {
      id: "h1_container".to_string(),
      direction: ContainerDirection::Vertical,
    },
    UiCommand::Text {
      content: "Hello World".to_string(),
      style: TextStyle::Heading1,
    },
    UiCommand::EndContainer,
  ];

  // Emit the template instruction
  sir.emit(Insn::Template {
    id: template_id,
    name: Some(interner.intern("hello_template")),
    ty_id,
    commands,
  });

  // Add a #dom directive to render it
  let dom_name = interner.intern("dom");
  sir.emit(Insn::Directive {
    name: dom_name,
    value: template_id,
    ty_id,
  });

  // Generate ARM64 code
  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // Verify code was generated
  assert!(!artifact.code.is_empty(), "Should generate ARM64 code");
  assert!(codegen.has_templates, "Should have templates");

  // Generate Mach-O binary
  let macho_binary = codegen.generate_macho(artifact);
  assert!(!macho_binary.is_empty(), "Should generate Mach-O binary");

  // Verify the binary contains expected markers
  verify_binary_structure(&macho_binary);

  // Save to temp file for manual inspection if needed
  if std::env::var("SAVE_TEST_BINARY").is_ok() {
    let temp_path = "/tmp/test_template.dylib";
    fs::write(temp_path, &macho_binary).unwrap();
    println!("Saved test binary to: {}", temp_path);
  }

  println!(
    "Successfully generated {} byte template binary",
    macho_binary.len()
  );
}

/// Verify the binary contains expected structures
fn verify_binary_structure(binary: &[u8]) {
  // Check for Mach-O magic number
  assert!(binary.len() >= 4);
  let magic = u32::from_le_bytes([binary[0], binary[1], binary[2], binary[3]]);
  assert!(
    magic == 0xFEEDFACF || magic == 0xCFFAEDFE,
    "Should have valid Mach-O magic number"
  );

  // Check for _zo_ui_entry_point symbol
  let entry_point = b"_zo_ui_entry_point";
  let has_entry = binary
    .windows(entry_point.len())
    .any(|window| window == entry_point);
  assert!(has_entry, "Should contain _zo_ui_entry_point symbol");

  // Check for expected strings from our template
  let hello_str = b"Hello World\0";
  let has_hello = binary
    .windows(hello_str.len())
    .any(|window| window == hello_str);
  assert!(has_hello, "Should contain 'Hello World' string");

  let container_str = b"h1_container\0";
  let has_container = binary
    .windows(container_str.len())
    .any(|window| window == container_str);
  assert!(has_container, "Should contain 'h1_container' string");
}

/// Test template data structure alignment and layout
#[test]
fn test_template_memory_layout() {
  let interner = Interner::new();
  let mut sir = Sir::new();

  // Create a template with various command types
  let commands = vec![
    UiCommand::BeginContainer {
      id: "main".to_string(),
      direction: ContainerDirection::Horizontal,
    },
    UiCommand::Text {
      content: "Label".to_string(),
      style: TextStyle::Normal,
    },
    UiCommand::Button {
      id: 42,
      content: "Click".to_string(),
    },
    UiCommand::TextInput {
      id: 100,
      placeholder: "Enter text".to_string(),
      value: "".to_string(),
    },
    UiCommand::EndContainer,
  ];

  sir.emit(Insn::Template {
    id: ValueId(1),
    name: None,
    ty_id: TyId(0),
    commands: commands.clone(),
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // The generated code should have proper alignment
  // Each command should be exactly 16 bytes
  let expected_command_bytes = commands.len() * 16;
  let header_bytes = 8; // count + padding
  let min_size = header_bytes + expected_command_bytes;

  assert!(
    artifact.code.len() >= min_size,
    "Generated code should be at least {} bytes for {} commands",
    min_size,
    commands.len()
  );

  println!("Memory layout test passed with {} commands", commands.len());
}

/// Test that multiple templates can coexist
#[test]
fn test_multiple_templates() {
  let mut interner = Interner::new();
  let mut sir = Sir::new();

  // First template
  sir.emit(Insn::Template {
    id: ValueId(1),
    name: Some(interner.intern("template1")),
    ty_id: TyId(0),
    commands: vec![UiCommand::Text {
      content: "Template 1".to_string(),
      style: TextStyle::Heading2,
    }],
  });

  // Second template
  sir.emit(Insn::Template {
    id: ValueId(2),
    name: Some(interner.intern("template2")),
    ty_id: TyId(0),
    commands: vec![UiCommand::Text {
      content: "Template 2".to_string(),
      style: TextStyle::Heading3,
    }],
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);
  let macho = codegen.generate_macho(artifact);

  // Both template strings should be present
  let t1 = b"Template 1\0";
  let has_t1 = macho.windows(t1.len()).any(|w| w == t1);
  assert!(has_t1, "Should contain first template text");

  let t2 = b"Template 2\0";
  let has_t2 = macho.windows(t2.len()).any(|w| w == t2);
  assert!(has_t2, "Should contain second template text");

  println!("Multiple templates test passed");
}
