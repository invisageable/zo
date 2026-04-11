use crate::ARM64Gen;

use zo_interner::Interner;
use zo_sir::{Insn, Sir};
use zo_ty::TyId;
use zo_ui_protocol::{Attr, ElementTag, UiCommand};
use zo_value::ValueId;

fn div_container(id: &str) -> UiCommand {
  UiCommand::Element {
    tag: ElementTag::Div,
    attrs: vec![Attr::str_prop("data-id", id)],
    self_closing: false,
  }
}

fn h1(content: &str) -> Vec<UiCommand> {
  vec![
    UiCommand::Element {
      tag: ElementTag::H1,
      attrs: vec![Attr::str_prop("data-id", "h1_0")],
      self_closing: false,
    },
    UiCommand::Text(content.into()),
    UiCommand::EndElement,
  ]
}

fn button(id: u32, label: &str) -> Vec<UiCommand> {
  vec![
    UiCommand::Element {
      tag: ElementTag::Button,
      attrs: vec![Attr::parse_prop("data-id", &id.to_string())],
      self_closing: false,
    },
    UiCommand::Text(label.into()),
    UiCommand::EndElement,
  ]
}

#[test]
fn test_template_with_ui_commands() {
  let interner = Interner::new();
  let mut sir = Sir::new();

  let template_id = ValueId(2);
  let ty_id = TyId(0);

  // Create a template: <div><h1>Hello, World!</h1></div>
  let mut commands = vec![div_container("container_0")];

  commands.extend(h1("Hello, World!"));
  commands.push(UiCommand::EndElement);

  sir.emit(Insn::Template {
    id: template_id,
    name: None,
    ty_id,
    commands,
    bindings: vec![],
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty(), "Should generate code");

  // Verify that template data was stored.
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

  // Simple template with one <p> text element.
  let commands = vec![
    UiCommand::Element {
      tag: ElementTag::P,
      attrs: vec![Attr::str_prop("data-id", "p_0")],
      self_closing: false,
    },
    UiCommand::Text("Test Text".into()),
    UiCommand::EndElement,
  ];

  sir.emit(Insn::Template {
    id: template_id,
    name: None,
    ty_id,
    commands,
    bindings: vec![],
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  assert!(!artifact.code.is_empty());

  println!(
    "Generated {} bytes for single text element",
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
    commands: vec![UiCommand::EndElement], // Minimal command
    bindings: vec![],
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);

  // Generate Mach-O and verify it has the entry point
  let macho = codegen.generate_macho(artifact);

  assert!(!macho.is_empty(), "Should generate Mach-O binary");

  // The Mach-O should contain the _zo_ui_entry_point symbol
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

  // Template: <div><button>Click Me</button><span>Button Label</span></div>
  let mut commands = vec![div_container("main")];

  commands.extend(button(1, "Click Me"));
  commands.extend(vec![
    UiCommand::Element {
      tag: ElementTag::Span,
      attrs: vec![Attr::str_prop("data-id", "span_0")],
      self_closing: false,
    },
    UiCommand::Text("Button Label".into()),
    UiCommand::EndElement,
  ]);
  commands.push(UiCommand::EndElement);

  sir.emit(Insn::Template {
    id: template_id,
    name: Some(interner.intern("button_template")),
    ty_id,
    commands,
    bindings: vec![],
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
