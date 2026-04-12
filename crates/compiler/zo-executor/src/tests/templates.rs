use crate::Executor;
use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;

// === TEMPLATE DECLARATION ===

#[test]
fn test_template_fragment_emits_template_sir() {
  assert_sir_structure(
    r#"fun main() {
  imu view: </> ::= <>hello</>;
}"#,
    |sir| {
      let has_template = sir.iter().any(|i| matches!(i, Insn::Template { .. }));

      assert!(has_template, "template fragment should emit Template SIR");
    },
  );
}

#[test]
fn test_template_named_tag_emits_template_sir() {
  assert_sir_structure(
    r#"fun main() {
  imu view: </> ::= <h1>hello</h1>;
}"#,
    |sir| {
      let has_template = sir.iter().any(|i| matches!(i, Insn::Template { .. }));

      assert!(has_template, "named tag template should emit Template SIR");
    },
  );
}

#[test]
fn test_template_var_registered() {
  let source = r#"fun main() {
  imu view: </> ::= <>hello</>;
  #dom view;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "template var should be registered: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === TEMPLATE INTERPOLATION ===

#[test]
fn test_template_interp_str_variable() {
  let source = r#"fun main() {
  imu name: str = "world";
  imu view: </> ::= <>hello, {name}!</>;
  #dom view;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "str interpolation should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_template_interp_int_variable() {
  let source = r#"fun main() {
  imu count: int = 42;
  imu view: </> ::= <>count: {count}</>;
  #dom view;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "int interpolation should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_template_interp_multiple_vars() {
  let source = r#"fun main() {
  imu a: str = "hello";
  imu b: str = "world";
  imu view: </> ::= <>{a}, {b}!</>;
  #dom view;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "multi-var interpolation should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

#[test]
fn test_template_interp_named_tag() {
  let source = r#"fun main() {
  imu name: str = "world";
  imu view: </> ::= <h1>hello, {name}!</h1>;
  #dom view;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "named tag interpolation should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// === TEMPLATE INTERPOLATION ERRORS ===

#[test]
fn test_template_interp_undefined_variable() {
  assert_execution_error(
    r#"fun main() {
  imu view: </> ::= <>{unknown}</>;
  #dom view;
}"#,
    ErrorKind::UndefinedVariable,
  );
}

#[test]
fn test_template_interp_empty_braces() {
  assert_execution_error(
    r#"fun main() {
  imu view: </> ::= <>{}</>;
  #dom view;
}"#,
    ErrorKind::ExpectedExpression,
  );
}

// === ATTRIBUTE INTERPOLATION ===

#[test]
fn test_template_attr_interpolation() {
  let source = r#"fun main() {
  imu src: str = "logo.png";
  imu view: </> ::= <img src={src} />;
  #dom view;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (_, _, _, _) = executor.execute();
  let errors = collect_errors();

  assert!(
    errors.is_empty(),
    "attribute interpolation should not error: {:?}",
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

/// Walk a `Vec<Insn>`, find the first `Insn::Template`, and
/// return the attributes of its first `UiCommand::Element`.
fn first_element_attrs(sir: &[Insn]) -> Vec<zo_ui_protocol::Attr> {
  use zo_ui_protocol::UiCommand;

  for insn in sir {
    if let Insn::Template { commands, .. } = insn {
      for cmd in commands {
        if let UiCommand::Element { attrs, .. } = cmd {
          return attrs.clone();
        }
      }
    }
  }

  Vec::new()
}

/// Look up a `Prop` attribute by name, returning its display
/// string value. Ignores `Event`, `Style`, `Dynamic` variants.
fn prop_value(attrs: &[zo_ui_protocol::Attr], name: &str) -> Option<String> {
  use zo_ui_protocol::Attr;

  for attr in attrs {
    if let Attr::Prop { name: n, value } = attr
      && n == name
    {
      return Some(value.to_display());
    }
  }

  None
}

#[test]
fn test_template_attr_shorthand_string() {
  assert_sir_structure(
    r#"fun main() {
  imu src: str = "logo.png";
  imu view: </> ::= <img {src} />;
  #dom view;
}"#,
    |sir| {
      let attrs = first_element_attrs(sir);

      assert_eq!(
        prop_value(&attrs, "src").as_deref(),
        Some("logo.png"),
        "shorthand `{{src}}` should resolve to the local's value"
      );
    },
  );
}

#[test]
fn test_template_attr_shorthand_numeric() {
  assert_sir_structure(
    r#"fun main() {
  imu width: int = 128;
  imu view: </> ::= <img src="a.png" {width} />;
  #dom view;
}"#,
    |sir| {
      let attrs = first_element_attrs(sir);

      assert_eq!(
        prop_value(&attrs, "width").as_deref(),
        Some("128"),
        "shorthand `{{width}}` should stringify the int value"
      );
    },
  );
}

#[test]
fn test_template_attr_shorthand_undefined_is_empty() {
  // `ghost` is not declared — the shorthand resolves to an
  // empty string rather than panicking. Matches text
  // interpolation's silently-empty semantics.
  assert_sir_structure(
    r#"fun main() {
  imu view: </> ::= <img {ghost} />;
  #dom view;
}"#,
    |sir| {
      let attrs = first_element_attrs(sir);

      assert_eq!(
        prop_value(&attrs, "ghost").as_deref(),
        Some(""),
        "undefined shorthand should produce an empty prop"
      );
    },
  );
}

#[test]
fn test_template_attr_interp_string_single_var() {
  assert_sir_structure(
    r#"fun main() {
  imu name: str = "kayode";
  imu view: </> ::= <img src="a.png" alt="a picture of {name}" />;
  #dom view;
}"#,
    |sir| {
      let attrs = first_element_attrs(sir);

      assert_eq!(
        prop_value(&attrs, "alt").as_deref(),
        Some("a picture of kayode"),
        "attr interp with one variable should splice in its value"
      );
    },
  );
}

#[test]
fn test_template_attr_interp_string_multi_vars() {
  assert_sir_structure(
    r#"fun main() {
  imu first: str = "johnny";
  imu last: str = "appleseed";
  imu view: </> ::= <img src="a.png" alt="{first} {last}" />;
  #dom view;
}"#,
    |sir| {
      let attrs = first_element_attrs(sir);

      assert_eq!(
        prop_value(&attrs, "alt").as_deref(),
        Some("johnny appleseed"),
        "attr interp with multiple variables should concatenate"
      );
    },
  );
}

// === REACTIVE ATTRIBUTE BINDINGS (R3) ===

/// Walk `sir` for the first `Insn::Template` and return a
/// clone of its bindings struct so tests can assert on both
/// `text` and `attrs` lists.
fn first_template_bindings(sir: &[Insn]) -> zo_sir::TemplateBindings {
  for insn in sir {
    if let Insn::Template { bindings, .. } = insn {
      return bindings.clone();
    }
  }

  zo_sir::TemplateBindings::default()
}

#[test]
fn test_template_attr_binding_emitted_for_mut_shorthand() {
  assert_sir_structure(
    r#"fun main() {
  mut src: str = "/a.png";
  imu view: </> ::= <img {src} />;
  #dom view;
}"#,
    |sir| {
      let bindings = first_template_bindings(sir);

      assert_eq!(
        bindings.attrs.len(),
        1,
        "mut shorthand should record one attribute binding"
      );

      let (_, attr) = &bindings.attrs[0];

      match attr {
        zo_ui_protocol::Attr::Dynamic { name, initial, .. } => {
          assert_eq!(name, "src");

          if let zo_ui_protocol::PropValue::Str(s) = initial {
            assert_eq!(s, "/a.png");
          } else {
            panic!("expected Str initial, got {:?}", initial);
          }
        }
        other => panic!("expected Attr::Dynamic, got {:?}", other),
      }
    },
  );
}

#[test]
fn test_template_attr_binding_emitted_for_mut_equal_form() {
  assert_sir_structure(
    r#"fun main() {
  mut src: str = "/a.png";
  imu view: </> ::= <img src={src} />;
  #dom view;
}"#,
    |sir| {
      let bindings = first_template_bindings(sir);

      assert_eq!(
        bindings.attrs.len(),
        1,
        "mut `src={{src}}` should record one attribute binding"
      );

      let (_, attr) = &bindings.attrs[0];

      assert!(
        matches!(attr, zo_ui_protocol::Attr::Dynamic { .. }),
        "binding should store an `Attr::Dynamic`"
      );
    },
  );
}

#[test]
fn test_template_attr_no_binding_for_imu_source() {
  assert_sir_structure(
    r#"fun main() {
  imu src: str = "/a.png";
  imu view: </> ::= <img src={src} />;
  #dom view;
}"#,
    |sir| {
      let bindings = first_template_bindings(sir);

      assert!(
        bindings.attrs.is_empty(),
        "imu source should not emit attribute bindings"
      );

      // The emitted attribute should be a plain `Prop`, not
      // `Dynamic`.
      let attrs = first_element_attrs(sir);
      let src_attr = attrs
        .iter()
        .find(|a| a.name() == "src")
        .expect("should find src attr");

      assert!(
        matches!(src_attr, zo_ui_protocol::Attr::Prop { .. }),
        "imu source should emit Prop, not Dynamic"
      );
    },
  );
}

#[test]
fn test_template_attr_binding_cmd_idx_points_at_element() {
  assert_sir_structure(
    r#"fun main() {
  mut src: str = "/a.png";
  imu view: </> ::= <img {src} />;
  #dom view;
}"#,
    |sir| {
      let bindings = first_template_bindings(sir);

      assert_eq!(bindings.attrs.len(), 1);

      let (cmd_idx, _) = bindings.attrs[0];

      // Find the template and verify `cmd_idx` points at an
      // `Element` command.
      for insn in sir {
        if let Insn::Template { commands, .. } = insn {
          assert!(
            matches!(
              commands.get(cmd_idx),
              Some(zo_ui_protocol::UiCommand::Element { .. })
            ),
            "binding cmd_idx should point at the Element command"
          );

          return;
        }
      }

      panic!("no Template instruction found");
    },
  );
}

#[test]
fn test_template_attr_eager_expr_form_regression() {
  // Regression: the existing `attr={ident}` form must keep
  // working alongside the new shorthand and interp forms.
  assert_sir_structure(
    r#"fun main() {
  imu src: str = "logo.png";
  imu view: </> ::= <img src={src} />;
  #dom view;
}"#,
    |sir| {
      let attrs = first_element_attrs(sir);

      assert_eq!(
        prop_value(&attrs, "src").as_deref(),
        Some("logo.png"),
        "eager `src={{src}}` form should still resolve"
      );
    },
  );
}

// === EVENT HANDLERS ===

#[test]
fn test_event_attribute_with_inline_closure() {
  use zo_ui_protocol::UiCommand;

  assert_sir_structure(
    r#"fun main() {
  imu app: </> ::= <>
    <button @click={fn() => showln("clicked")}>click</button>
  </>;
  #dom app;
}"#,
    |sir| {
      // Find the Template instruction and check its Event command.
      for insn in sir {
        if let Insn::Template { commands, .. } = insn {
          let event_cmd = commands
            .iter()
            .find(|c| matches!(c, UiCommand::Event { .. }));

          assert!(
            event_cmd.is_some(),
            "should emit Event command for @click with closure",
          );

          if let Some(UiCommand::Event { handler, .. }) = event_cmd {
            assert!(
              handler.starts_with("__closure_"),
              "handler should be closure name, got: {handler}",
            );
          }

          return;
        }
      }

      panic!("no Template instruction found in SIR");
    },
  );
}

// === DOM DIRECTIVE ===

#[test]
fn test_dom_directive_emits_insn() {
  assert_sir_structure(
    r#"fun main() {
  imu view: </> ::= <>hello</>;
  #dom view;
}"#,
    |sir| {
      let has_directive =
        sir.iter().any(|i| matches!(i, Insn::Directive { .. }));

      assert!(has_directive, "#dom should emit Insn::Directive: {sir:#?}");
    },
  );
}

// === #HTML DIRECTIVE ===

/// Format the commands of the first `Insn::Template` as a
/// `{ tag content }` stream that's easy to eyeball in test
/// output. Used by the smoke test below with `--nocapture` so
/// the user can visually verify the splice landed correctly.
fn format_template_commands(sir: &[Insn]) -> String {
  use zo_ui_protocol::UiCommand;

  let mut out = String::new();

  for insn in sir {
    if let Insn::Template { commands, .. } = insn {
      out.push_str("Template commands:\n");

      for (idx, cmd) in commands.iter().enumerate() {
        match cmd {
          UiCommand::Element {
            tag,
            attrs,
            self_closing,
          } => {
            out.push_str(&format!(
              "  [{idx}] Element {{ tag: {:?}, attrs: {}, self_closing: {} }}\n",
              tag,
              attrs.len(),
              self_closing,
            ));
          }
          UiCommand::EndElement => {
            out.push_str(&format!("  [{idx}] EndElement\n"));
          }
          UiCommand::Text(s) => {
            out.push_str(&format!("  [{idx}] Text({:?})\n", s));
          }
          UiCommand::Event { handler, .. } => {
            out.push_str(&format!("  [{idx}] Event({handler})\n"));
          }
          UiCommand::StyleSheet { .. } => {
            out.push_str(&format!("  [{idx}] StyleSheet\n"));
          }
        }
      }

      return out;
    }
  }

  out.push_str("(no Template insn found)\n");
  out
}

#[test]
fn test_html_directive_diagnose_sub_parse() {
  // Call parse_raw_html directly to see what the sub-parse
  // returns for the exact string used in 064.
  let commands =
    crate::html_inline::parse_raw_html("here's some <strong>html!!!</strong>");

  eprintln!("sub-parse returned {} commands:", commands.len());

  for (i, cmd) in commands.iter().enumerate() {
    eprintln!("  [{i}] {:?}", cmd);
  }
}

/// Same zo source as `zo-how-zo/wip/064-zsx-html-directive.zo`
/// — mirrored here as an executor-level smoke test so the
/// rendered command stream is visible via `--nocapture` and
/// checked in CI. Run with:
///
/// ```sh
/// cargo test -p zo-executor test_html_directive_smoke -- --nocapture
/// ```
///
/// Expected output (order matters):
///
/// ```text
/// Template commands:
///   [0] Element { tag: P, attrs: 1, self_closing: false }
///   [1] Text("here's some ")
///   [2] Element { tag: Custom("strong"), attrs: 1, self_closing: false }
///   [3] Text("html!!!")
///   [4] EndElement
///   [5] EndElement
/// ```
#[test]
fn test_html_directive_smoke() {
  use zo_ui_protocol::{ElementTag, UiCommand};

  let source = r#"fun main() {
  imu strong: str = "here's some <strong>html!!!</strong>";
  imu paragraph: </> ::= <p>{#html strong}</p>;
  #dom paragraph;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (sir, _, _, _) = executor.execute();

  // Print the rendered template commands. Visible with
  // `--nocapture`.
  println!("{}", format_template_commands(&sir.instructions));

  // Programmatic structural assertions.
  let template_commands = sir
    .instructions
    .iter()
    .find_map(|i| match i {
      Insn::Template { commands, .. } => Some(commands),
      _ => None,
    })
    .expect("should find one Template insn");

  // Expect: Element(P) — Text("here's some ") — Element(Custom("strong"))
  //       — Text("html!!!") — EndElement — EndElement
  assert!(
    template_commands.len() >= 6,
    "expected at least 6 commands, got {}: {:#?}",
    template_commands.len(),
    template_commands
  );

  // First command is the enclosing <p>.
  assert!(
    matches!(
      &template_commands[0],
      UiCommand::Element {
        tag: ElementTag::P,
        self_closing: false,
        ..
      }
    ),
    "first command should be <p>, got {:?}",
    template_commands[0]
  );

  // Somewhere in the stream we must find an Element with a
  // `Custom("strong")` tag — that's the spliced HTML.
  let has_strong = template_commands.iter().any(|c| {
    matches!(
      c,
      UiCommand::Element { tag: ElementTag::Custom(name), .. } if name == "strong"
    )
  });

  assert!(
    has_strong,
    "spliced <strong> element missing from command stream: {:#?}",
    template_commands
  );

  // And the spliced text "html!!!" must appear as a TextNode.
  let has_html_text = template_commands
    .iter()
    .any(|c| matches!(c, UiCommand::Text(s) if s.contains("html!!!")));

  assert!(
    has_html_text,
    "spliced text `html!!!` missing: {:#?}",
    template_commands
  );

  // Two EndElements (one for the spliced <strong>, one for
  // the enclosing <p>).
  let end_count = template_commands
    .iter()
    .filter(|c| matches!(c, UiCommand::EndElement))
    .count();

  assert!(
    end_count >= 2,
    "expected at least 2 EndElements, got {end_count}: {:#?}",
    template_commands
  );
}

#[test]
fn test_html_directive_rejects_mut_source() {
  let source = r#"fun main() {
  mut strong: str = "<strong>html</strong>";
  imu paragraph: </> ::= <p>{#html strong}</p>;
  #dom paragraph;
}"#;

  let tokenizer = Tokenizer::new(source);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let executor = Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let _ = executor.execute();
  let errors = collect_errors();

  assert!(
    !errors.is_empty(),
    "#html on a mut source should produce a diagnostic"
  );
}
