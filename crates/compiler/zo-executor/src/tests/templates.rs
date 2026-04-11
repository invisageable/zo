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

  let (_, _, _) = executor.execute();
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

  let (_, _, _) = executor.execute();
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

  let (_, _, _) = executor.execute();
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

  let (_, _, _) = executor.execute();
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

  let (_, _, _) = executor.execute();
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

  let (_, _, _) = executor.execute();
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
