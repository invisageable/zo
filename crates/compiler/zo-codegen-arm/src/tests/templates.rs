use crate::ARM64Gen;

use zo_interner::Interner;
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_ui_protocol::{ContainerDirection, TextStyle, UiCommand};
use zo_value::ValueId;

#[test]
fn test_template_with_ui_commands() {
  let interner = Interner::new();
  let mut sir = Sir::new();

  let template_id = ValueId(2);
  let ty_id = TyId(0);

  // Create a template with actual UI commands
  let commands = vec![
    UiCommand::BeginContainer {
      id: "container_0".to_string(),
      direction: ContainerDirection::Vertical,
    },
    UiCommand::Text {
      content: "Hello, World!".to_string(),
      style: TextStyle::Heading1,
    },
    UiCommand::EndContainer,
  ];

  sir.emit(Insn::Template {
    id: template_id,
    name: None,
    ty_id,
    commands,
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty(), "Should generate code");

  // The generated code should contain:
  // 1. _zo_ui_entry_point function
  // 2. Template data with proper layout
  // 3. String table with "container_0" and "Hello, World!"

  // Verify that template data was stored
  assert!(codegen.has_templates, "Should have templates flag set");

  println!(
    "Generated {} bytes for template with commands",
    artifact.code.len()
  );
}

#[test]
fn test_template_data_layout() {
  let interner = Interner::new();
  let mut sir = Sir::new();

  let template_id = ValueId(1);
  let ty_id = TyId(0);

  // Simple template with one text command
  let commands = vec![UiCommand::Text {
    content: "Test Text".to_string(),
    style: TextStyle::Paragraph,
  }];

  sir.emit(Insn::Template {
    id: template_id,
    name: None,
    ty_id,
    commands,
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // The data layout should be:
  // [u32 count=1][u32 padding]
  // [u32 type=2][u32 padding][u64 data_ptr]
  // [command data structure]
  // [string table with "Test Text\0"]

  assert!(!artifact.code.is_empty());

  // Find the template data in the generated code
  // It should start after the code section
  // We can validate the structure by checking specific byte patterns

  println!(
    "Generated {} bytes for single text command",
    artifact.code.len()
  );
}

#[test]
fn test_template_entry_point_export() {
  let interner = Interner::new();
  let mut sir = Sir::new();

  let template_id = ValueId(1);
  let ty_id = TyId(0);

  sir.emit(Insn::Template {
    id: template_id,
    name: None,
    ty_id,
    commands: vec![UiCommand::EndContainer], // Minimal command
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // Generate Mach-O and verify it has the entry point
  let macho = codegen.generate_macho(artifact);

  assert!(!macho.is_empty(), "Should generate Mach-O binary");

  // The Mach-O should contain the _zo_ui_entry_point symbol
  // We can check for the symbol name in the binary
  let entry_point = b"_zo_ui_entry_point";
  let has_entry = macho
    .windows(entry_point.len())
    .any(|window| window == entry_point);

  assert!(has_entry, "Should export _zo_ui_entry_point symbol");

  println!("Generated {} bytes Mach-O with entry point", macho.len());
}

#[test]
fn test_template_with_dom_directive() {
  let mut interner = Interner::new();
  let mut sir = Sir::new();

  let template_id = ValueId(1);
  let ty_id = TyId(0);

  // Create template with button and text
  let commands = vec![
    UiCommand::BeginContainer {
      id: "main".to_string(),
      direction: ContainerDirection::Horizontal,
    },
    UiCommand::Button {
      id: 1,
      content: "Click Me".to_string(),
    },
    UiCommand::Text {
      content: "Button Label".to_string(),
      style: TextStyle::Normal,
    },
    UiCommand::EndContainer,
  ];

  sir.emit(Insn::Template {
    id: template_id,
    name: Some(interner.intern("button_template")),
    ty_id,
    commands,
  });

  // Add #dom directive
  let dom_name = interner.intern("dom");
  sir.emit(Insn::Directive {
    name: dom_name,
    value: template_id,
    ty_id,
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty(), "Should generate code");
  assert!(codegen.has_templates, "Should have templates");

  println!(
    "Generated {} bytes with #dom directive",
    artifact.code.len()
  );
}
