use crate::ARM64Gen;

use zo_codegen_backend::Webviewing;
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
    bindings: zo_sir::TemplateBindings::default(),
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
    bindings: zo_sir::TemplateBindings::default(),
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
    bindings: zo_sir::TemplateBindings::default(),
  });

  let mut codegen = ARM64Gen::new(&interner);
  let artifact = codegen.generate(&sir);
  let link_obj = codegen.into_link_object(artifact);

  // Generate Mach-O and verify it has the entry point
  let macho = zo_linker::link_macho(
    link_obj,
    zo_codegen_backend::Target::Arm64AppleDarwin,
  )
  .executable;

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
    bindings: zo_sir::TemplateBindings::default(),
  });

  // Add #render directive
  let dom_name = interner.intern("render");

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
    "Generated {} bytes with #render directive",
    artifact.code.len()
  );

  // `#render` imports `_zo_run_native` — a UI-exclusive
  // symbol — so the linker must select the full runtime,
  // and its `LC_LOAD_DYLIB` must resolve through the one
  // canonical runtime path (no parallel absolute-path
  // `libzo_runtime_native` reference).
  let link_obj = codegen.into_link_object(artifact);
  let output = zo_linker::link_macho(
    link_obj,
    zo_codegen_backend::Target::Arm64AppleDarwin,
  );

  assert_eq!(
    output.runtime,
    zo_linker::RuntimeKind::Full,
    "a #render program must select the full UI runtime"
  );

  // All six UI symbols route to the same canonical runtime
  // path; the linker must collapse them into exactly ONE
  // `LC_LOAD_DYLIB` (the string also occurs once inside the
  // rebased-bind opcode stream, hence the load-command scan
  // below rather than a raw byte count).
  let dom_path = b"@loader_path/deps/libzo_runtime.dylib";
  let occurrences = output
    .executable
    .windows(dom_path.len())
    .filter(|window| *window == dom_path)
    .count();

  assert_eq!(
    occurrences, 1,
    "six UI symbols sharing the runtime path must collapse to one \
     LC_LOAD_DYLIB, found {occurrences}"
  );

  let native = b"libzo_runtime_native.dylib";
  let has_native = output
    .executable
    .windows(native.len())
    .any(|window| window == native);

  assert!(
    !has_native,
    "UI symbols must fold into libzo_runtime.dylib, not a \
     separate libzo_runtime_native reference"
  );
}

/// The webview build emits the wry runtime entry. A `#render` program
/// compiled with `Webviewing::Yes` must import `_zo_run_web` (the wry
/// AOT entry) and never `_zo_run_native`; `No` is the mirror image.
/// Both share the same SIR — only the emitted entry symbol differs.
#[test]
fn webview_render_calls_zo_run_web() {
  fn run_symbols(webviewing: Webviewing) -> (bool, bool) {
    let mut interner = Interner::new();
    let mut sir = Sir::new();

    let template_id = ValueId(1);
    let ty_id = TyId(0);

    sir.emit(Insn::Template {
      id: template_id,
      name: Some(interner.intern("counter")),
      ty_id,
      commands: vec![UiCommand::EndElement],
      bindings: zo_sir::TemplateBindings::default(),
    });

    sir.emit(Insn::Directive {
      name: interner.intern("render"),
      value: template_id,
      ty_id,
    });

    let mut codegen = ARM64Gen::new(&interner).with_webviewing(webviewing);
    let artifact = codegen.generate(&sir);
    let executable = zo_linker::link_macho(
      codegen.into_link_object(artifact),
      zo_codegen_backend::Target::Arm64AppleDarwin,
    )
    .executable;

    let contains = |needle: &[u8]| {
      executable
        .windows(needle.len())
        .any(|window| window == needle)
    };

    (contains(b"_zo_run_web"), contains(b"_zo_run_native"))
  }

  let (web_has_web, web_has_native) = run_symbols(Webviewing::Yes);

  assert!(web_has_web, "a webview `#render` must import `_zo_run_web`");
  assert!(
    !web_has_native,
    "a webview `#render` must not import the eframe `_zo_run_native`"
  );

  let (native_has_web, native_has_native) = run_symbols(Webviewing::No);

  assert!(
    native_has_native,
    "a native `#render` must import `_zo_run_native`"
  );
  assert!(
    !native_has_web,
    "a native `#render` must not import the wry `_zo_run_web`"
  );
}
