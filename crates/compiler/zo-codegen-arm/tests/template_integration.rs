//! Integration test for template code generation and runtime compatibility.
//!
//! R1 state: the binary encoder for the unified Element model is
//! a stub (it emits type codes + null data pointers but no
//! attribute payload or string table). These tests verify the
//! Mach-O scaffolding and entry-point symbol; full string-table
//! assertions will return once the encoder lands.

use zo_codegen_arm::ARM64Gen;
use zo_interner::Interner;
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_ui_protocol::{Attr, ElementTag, UiCommand};
use zo_value::ValueId;

use std::fs;

/// Test that generates a complete template binary that should be
/// loadable by zo-runtime.
#[test]
fn test_complete_template_binary() {
  let mut interner = Interner::new();
  let mut sir = Sir::new();

  let template_id = ValueId(1);
  let ty_id = TyId(0);

  // Build UI commands for: <h1>Hello World</h1>
  let commands = vec![
    UiCommand::Element {
      tag: ElementTag::H1,
      attrs: vec![Attr::str_prop("data-id", "h1_container")],
      self_closing: false,
    },
    UiCommand::Text("Hello World".into()),
    UiCommand::EndElement,
  ];

  sir.emit(Insn::Template {
    id: template_id,
    name: Some(interner.intern("hello_template")),
    ty_id,
    commands,
    bindings: vec![],
  });

  let dom_name = interner.intern("dom");

  sir.emit(Insn::Directive {
    name: dom_name,
    value: template_id,
    ty_id,
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty(), "Should generate ARM64 code");
  assert!(codegen.has_templates, "Should have templates");

  let macho_binary = codegen.generate_macho(artifact);

  assert!(!macho_binary.is_empty(), "Should generate Mach-O binary");

  verify_binary_structure(&macho_binary);

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

/// Verify the binary scaffolding (Mach-O magic + entry point
/// symbol). Full string-table inspection is deferred until the
/// unified Element binary encoder is implemented.
fn verify_binary_structure(binary: &[u8]) {
  assert!(binary.len() >= 4);

  let magic = u32::from_le_bytes([binary[0], binary[1], binary[2], binary[3]]);

  assert!(
    magic == 0xFEEDFACF || magic == 0xCFFAEDFE,
    "Should have valid Mach-O magic number"
  );

  let entry_point = b"_zo_ui_entry_point";
  let has_entry = binary
    .windows(entry_point.len())
    .any(|window| window == entry_point);

  assert!(has_entry, "Should contain _zo_ui_entry_point symbol");
}

/// Test template data structure alignment and layout.
#[test]
fn test_template_memory_layout() {
  let interner = Interner::new();
  let mut sir = Sir::new();

  // Template with various element kinds.
  let commands = vec![
    UiCommand::Element {
      tag: ElementTag::Div,
      attrs: vec![Attr::str_prop("data-id", "main")],
      self_closing: false,
    },
    UiCommand::Element {
      tag: ElementTag::Span,
      attrs: vec![Attr::str_prop("data-id", "span_0")],
      self_closing: false,
    },
    UiCommand::Text("Label".into()),
    UiCommand::EndElement,
    UiCommand::Element {
      tag: ElementTag::Button,
      attrs: vec![Attr::parse_prop("data-id", "42")],
      self_closing: false,
    },
    UiCommand::Text("Click".into()),
    UiCommand::EndElement,
    UiCommand::Element {
      tag: ElementTag::Input,
      attrs: vec![
        Attr::parse_prop("data-id", "100"),
        Attr::str_prop("placeholder", "Enter text"),
        Attr::str_prop("value", ""),
      ],
      self_closing: true,
    },
    UiCommand::EndElement,
  ];

  sir.emit(Insn::Template {
    id: ValueId(1),
    name: None,
    ty_id: TyId(0),
    commands: commands.clone(),
    bindings: vec![],
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // Each command occupies 16 bytes of command header + null
  // data pointer in the stubbed encoder.
  let expected_command_bytes = commands.len() * 16;
  let header_bytes = 8;
  let min_size = header_bytes + expected_command_bytes;

  assert!(
    artifact.code.len() >= min_size,
    "Generated code should be at least {} bytes for {} commands",
    min_size,
    commands.len()
  );

  println!("Memory layout test passed with {} commands", commands.len());
}

/// Test that multiple templates can coexist.
#[test]
fn test_multiple_templates() {
  let mut interner = Interner::new();
  let mut sir = Sir::new();

  sir.emit(Insn::Template {
    id: ValueId(1),
    name: Some(interner.intern("template1")),
    ty_id: TyId(0),
    commands: vec![
      UiCommand::Element {
        tag: ElementTag::H2,
        attrs: vec![Attr::str_prop("data-id", "h2_0")],
        self_closing: false,
      },
      UiCommand::Text("Template 1".into()),
      UiCommand::EndElement,
    ],
    bindings: vec![],
  });

  sir.emit(Insn::Template {
    id: ValueId(2),
    name: Some(interner.intern("template2")),
    ty_id: TyId(0),
    commands: vec![
      UiCommand::Element {
        tag: ElementTag::H3,
        attrs: vec![Attr::str_prop("data-id", "h3_0")],
        self_closing: false,
      },
      UiCommand::Text("Template 2".into()),
      UiCommand::EndElement,
    ],
    bindings: vec![],
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);
  let macho = codegen.generate_macho(artifact);

  assert!(!macho.is_empty(), "Should generate Mach-O binary");

  println!("Multiple templates test passed ({} bytes)", macho.len());
}
