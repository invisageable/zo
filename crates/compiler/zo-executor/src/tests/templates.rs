use crate::Executor;
use crate::tests::common::{assert_execution_error, assert_sir_structure};

use zo_error::ErrorKind;
use zo_interner::Interner;
use zo_parser::Parser;
use zo_reporter::collect_errors;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ty_checker::TyChecker;
use zo_value::FunctionKind;

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
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();
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
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();
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
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();
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
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();
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
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();
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
  #render view;
}"#,
    ErrorKind::UndefinedVariable,
  );
}

#[test]
fn test_template_interp_empty_braces() {
  assert_execution_error(
    r#"fun main() {
  imu view: </> ::= <>{}</>;
  #render view;
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
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render view;
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
  #render app;
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
  #render view;
}"#,
    |sir| {
      let has_directive =
        sir.iter().any(|i| matches!(i, Insn::Directive { .. }));

      assert!(
        has_directive,
        "#render should emit Insn::Directive: {sir:#?}"
      );
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
  #render paragraph;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let sir = executor.execute().sir;

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
  #render paragraph;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  executor.execute();
  let errors = collect_errors();

  assert!(
    !errors.is_empty(),
    "#html on a mut source should produce a diagnostic"
  );
}

// === COMPUTED BINDINGS (compound `{expr}` interpolation) ===

#[test]
fn test_compound_interp_emits_computed_binding() {
  // A compound `{when count == 1 ? "x" : "y"}` interp must
  // (1) emit an `__interp_<n>` closure FunDef into the SIR
  // and (2) attach a `(cmd_idx, ComputedBinding { closure_
  // name, captures })` to the enclosing `Insn::Template`'s
  // `bindings.computed`. Without both, the runtime can't
  // recompute the text on each state change.
  let source = r#"fun main() {
  mut count: int = 0;
  imu view: </> ::= <p>{when count == 1 ? "x" : "y"}</p>;
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();
  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let sir = executor.execute().sir;

  // The Insn::Template must carry exactly one computed
  // binding for the `{when …}` interp.
  let computed = sir
    .instructions
    .iter()
    .find_map(|i| match i {
      Insn::Template { bindings, .. } => Some(&bindings.computed),
      _ => None,
    })
    .expect("Template insn missing");

  assert_eq!(
    computed.len(),
    1,
    "expected one computed binding, got {}",
    computed.len()
  );

  let (_cmd_idx, cb) = &computed[0];

  // Captures must list `count` (the only mut local
  // referenced in the brace).
  let count_sym = interner.symbol("count").expect("count must be interned");

  assert_eq!(cb.captures, vec![count_sym]);

  // The closure FunDef must be emitted with the same
  // symbol DCE relies on for liveness.
  let has_closure = sir.instructions.iter().any(|i| {
    matches!(
      i,
      Insn::FunDef {
        name,
        kind: FunctionKind::Closure { capture_count: 1 },
        ..
      } if *name == cb.closure_name
    )
  });

  assert!(has_closure, "compound interp closure must appear in SIR");
}

#[test]
fn test_compound_interp_emits_text_placeholder() {
  // The executor pushes a placeholder `UiCommand::Text`
  // for the compound interp so the runtime has a slot to
  // patch on each render. The cmd_idx in
  // `bindings.computed` must point at that exact slot.
  let source = r#"fun main() {
  mut count: int = 0;
  imu view: </> ::= <p>before {when count == 1 ? "x" : "y"} after</p>;
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();
  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let sir = executor.execute().sir;

  let (commands, computed) = sir
    .instructions
    .iter()
    .find_map(|i| match i {
      Insn::Template {
        commands, bindings, ..
      } => Some((commands, &bindings.computed)),
      _ => None,
    })
    .expect("Template insn missing");

  let (cmd_idx, _) = computed[0];

  // The bound slot must exist and must be a Text command.
  let cmd = commands
    .get(cmd_idx)
    .expect("computed binding cmd_idx out of range");

  assert!(
    matches!(cmd, zo_ui_protocol::UiCommand::Text(_)),
    "computed binding must point at a UiCommand::Text"
  );
}

#[test]
fn test_simple_ident_interp_uses_text_binding_not_computed() {
  // Regression: the simple-ident fast path
  // (`{count}`) MUST stay on `bindings.text`. If a future
  // refactor accidentally routed it through the compound
  // path, every `{count}` would lose its existing
  // reactive `mut` binding.
  let source = r#"fun main() {
  mut count: int = 0;
  imu view: </> ::= <p>{count}</p>;
  #render view;
}"#;

  let mut interner = Interner::new();
  let tokenizer = Tokenizer::new(source, &mut interner);
  let tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();
  let mut ty_checker = TyChecker::new();

  let executor = Executor::new(
    &parsing.tree,
    &mut interner,
    &tokenization.literals,
    &mut ty_checker,
  );

  let sir = executor.execute().sir;

  let bindings = sir
    .instructions
    .iter()
    .find_map(|i| match i {
      Insn::Template { bindings, .. } => Some(bindings),
      _ => None,
    })
    .expect("Template insn missing");

  assert_eq!(bindings.text.len(), 1, "{{count}} must be a text binding");
  assert!(
    bindings.computed.is_empty(),
    "{{count}} must NOT route through computed bindings"
  );
}

#[test]
fn test_template_list_binding_extracted_from_map_call() {
  use zo_sir::ListItemCmd;

  assert_sir_structure(
    r#"fun main() {
  imu items: []str = ["a", "b"];
  imu view: </> ::= <ul>{items.map(fn(t) =:> <li>{t}</li>)}</ul>;
  #render view;
}"#,
    |sir| {
      let bindings = first_template_bindings(sir);

      assert_eq!(
        bindings.list.len(),
        1,
        "`.map(...)` inside template interp must register one list binding"
      );

      let (_, lb) = &bindings.list[0];
      let recipe = &lb.item_template;

      // Recipe shape: Element(li) → TextFromItem → EndElement.
      assert_eq!(
        recipe.len(),
        3,
        "single-tag wrapper with one {{t}} interp produces 3 recipe steps"
      );

      assert!(
        matches!(
          recipe[0],
          ListItemCmd::Element {
            tag: zo_ui_protocol::ElementTag::Li,
            ..
          }
        ),
        "recipe[0] must open <li>"
      );
      assert!(
        matches!(recipe[1], ListItemCmd::TextFromItem),
        "recipe[1] must substitute item value"
      );
      assert!(
        matches!(recipe[2], ListItemCmd::EndElement),
        "recipe[2] must close </li>"
      );
    },
  );
}

#[test]
fn test_template_list_binding_emits_template_insn() {
  // The Template Insn must still get emitted (and its
  // `commands` populated with the wrapping `<ul>...</ul>`)
  // even when the interp expands to a list binding. Without
  // this, the runtime sees zero UI commands and the window
  // never opens.
  use zo_sir::Insn;

  assert_sir_structure(
    r#"fun main() {
  imu items: []str = ["a", "b"];
  imu view: </> ::= <ul>{items.map(fn(t) =:> <li>{t}</li>)}</ul>;
  #render view;
}"#,
    |sir| {
      let template = sir
        .iter()
        .find_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .expect("Template Insn missing — list-binding path swallowed it");

      let n = template.len();
      assert!(
        n >= 3,
        "expected at least Element(ul) + Text(placeholder) + EndElement, got {n}",
      );
    },
  );
}

#[test]
fn component_function_splices_in_tag_position() {
  // `header` is a `-> </>` function; `<header />` must splice its
  // template commands into `page` exactly like a `::=` variable
  // component — an `<h1>` element and its text land between the
  // `<div>`'s open and close.
  assert_sir_structure(
    r#"
fun header() -> </> {
  return <h1>hello</h1>;
}

fun main() {
  imu page ::= <div><header /></div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::{ElementTag, UiCommand};

      // The LAST template is `page` (the component's own template
      // is emitted first, inside `header`'s body).
      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let has_h1 = page
        .iter()
        .any(|c| matches!(c, UiCommand::Element { tag, .. } if *tag == ElementTag::H1));
      let has_text = page
        .iter()
        .any(|c| matches!(c, UiCommand::Text(t) if t == "hello"));

      assert!(has_h1, "spliced <h1> missing from page: {page:#?}");
      assert!(has_text, "spliced text missing from page: {page:#?}");
    },
  );
}

#[test]
fn unknown_component_tag_stays_a_plain_element() {
  // A tag that resolves to neither a template variable nor a
  // component function falls through to plain-element emission —
  // same behavior as before the component registry.
  assert_sir_structure(
    r#"
fun main() {
  imu page ::= <div><mystery /></div>;

  #render page;
}"#,
    |sir| {
      let has_template = sir.iter().any(|i| matches!(i, Insn::Template { .. }));

      assert!(has_template, "page template should still build");
    },
  );
}

#[test]
fn component_function_tail_form_splices_too() {
  // Same component, tail expression instead of explicit `return` —
  // the fragment is the body's value either way.
  assert_sir_structure(
    r#"
fun header() -> </> {
  <h1>hello</h1>
}

fun main() {
  imu page ::= <div><header /></div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::{ElementTag, UiCommand};

      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let has_h1 = page
        .iter()
        .any(|c| matches!(c, UiCommand::Element { tag, .. } if *tag == ElementTag::H1));

      assert!(has_h1, "tail-form splice missing <h1>: {page:#?}");
    },
  );
}

#[test]
fn component_props_bake_per_instance() {
  // `<greeting name="..." />` re-executes the component body with
  // `name` bound — two instances bake two different texts.
  assert_sir_structure(
    r#"
fun greeting(name: str) -> </> {
  return <h1>hello, {name}!</h1>;
}

fun main() {
  imu page ::= <div>
    <greeting name="world" />
    <greeting name="zo" />
  </div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;

      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let texts: Vec<&str> = page
        .iter()
        .filter_map(|c| match c {
          UiCommand::Text(t) => Some(t.as_str()),
          _ => None,
        })
        .collect();

      assert!(
        texts.iter().any(|t| t.contains("world")),
        "first instance text missing: {texts:?}"
      );
      assert!(
        texts.iter().any(|t| t.contains("zo")),
        "second instance text missing: {texts:?}"
      );
    },
  );
}

#[test]
fn component_missing_prop_reports_argument_mismatch() {
  use super::common::assert_execution_error;

  use zo_error::ErrorKind;

  assert_execution_error(
    r#"
fun greeting(name: str) -> </> {
  return <h1>hello, {name}!</h1>;
}

fun main() {
  imu page ::= <div><greeting /></div>;

  #render page;
}"#,
    ErrorKind::ArgumentCountMismatch,
  );
}

#[test]
fn component_prop_from_braced_local() {
  // `name={user}` — an immutable local arrives as an eager
  // `Attr::Prop`; a mutable one as `Attr::Dynamic` whose
  // `initial` carries the same eager value. Both bind.
  assert_sir_structure(
    r#"
fun greeting(name: str) -> </> {
  return <h1>hello, {name}!</h1>;
}

fun main() {
  imu user := "world";
  mut who := "zo";

  imu page ::= <div>
    <greeting name={user} />
    <greeting name={who} />
  </div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;

      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let texts: Vec<&str> = page
        .iter()
        .filter_map(|c| match c {
          UiCommand::Text(t) => Some(t.as_str()),
          _ => None,
        })
        .collect();

      assert!(
        texts.iter().any(|t| t.contains("world")),
        "imu-local prop missing: {texts:?}"
      );
      assert!(
        texts.iter().any(|t| t.contains("zo")),
        "mut-local prop missing: {texts:?}"
      );
    },
  );
}

#[test]
fn nested_reactive_component_keeps_its_bindings() {
  // A component with its own `mut` used to lose its reactive
  // bindings at the splice (only `commands` were cloned). The
  // page template must carry the child's text binding, rebased to
  // the splice offset — the bound `UiCommand::Text` at that index
  // is the child's.
  assert_sir_structure(
    r#"
fun main() {
  mut count := 0;

  imu badge ::= <span>{count}</span>;
  imu page ::= <div><p>items:</p><badge /></div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;

      let (commands, bindings) = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template {
            commands, bindings, ..
          } => Some((commands, bindings)),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let all: Vec<_> = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { bindings, .. } => Some(bindings.clone()),
          _ => None,
        })
        .collect();

      assert!(
        !bindings.text.is_empty(),
        "child's text binding lost at splice; all templates: {all:#?}"
      );

      // The rebased index must point at a Text command (the
      // child's `{count}` slot), not at arbitrary commands.
      for (cmd_idx, _) in &bindings.text {
        assert!(
          matches!(commands.get(*cmd_idx), Some(UiCommand::Text(_))),
          "binding index {cmd_idx} does not target a Text command: \
           {commands:#?}"
        );
      }
    },
  );
}

#[test]
fn twice_spliced_reactive_component_carries_both_bindings() {
  // Two instances of one reactive component: each splice rebases
  // the child's binding to its own offset, so the page carries
  // two text bindings at distinct indices. (The instances share
  // the `count` state cell — per-instance state is future work.)
  assert_sir_structure(
    r#"
fun main() {
  mut count := 0;

  imu badge ::= <span>{count}</span>;
  imu page ::= <div><badge /><badge /></div>;

  #render page;
}"#,
    |sir| {
      let bindings = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { bindings, .. } => Some(bindings),
          _ => None,
        })
        .next_back()
        .expect("page template");

      assert_eq!(
        bindings.text.len(),
        2,
        "each instance must carry its own rebased binding: {bindings:?}"
      );
      assert_ne!(
        bindings.text[0].0, bindings.text[1].0,
        "the two instances' bindings must target distinct commands"
      );
    },
  );
}

#[test]
fn slot_splices_children_at_the_marked_position() {
  // `<card …><p>body</p></card>` — children build in the parent's
  // scope and land exactly where the component body says
  // `<slot />`: between the card's heading and footer.
  assert_sir_structure(
    r#"
fun card(title: str) -> </> {
  return <div>
    <h2>{title}</h2>
    <slot />
    <p>footer</p>
  </div>;
}

fun main() {
  imu who := "zo";

  imu page ::= <card title="hi"><p>body {who}</p></card>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;

      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let texts: Vec<&str> = page
        .iter()
        .filter_map(|c| match c {
          UiCommand::Text(t) => Some(t.as_str()),
          _ => None,
        })
        .collect();

      let hi = texts.iter().position(|t| t.contains("hi"));
      let body = texts.iter().position(|t| t.contains("body zo"));
      let footer = texts.iter().position(|t| t.contains("footer"));

      assert!(
        hi.is_some() && body.is_some() && footer.is_some(),
        "missing slot pieces: {texts:?}"
      );
      assert!(
        hi < body && body < footer,
        "slot content out of position: {texts:?}"
      );
    },
  );
}

#[test]
fn slotless_component_appends_children_instead_of_dropping() {
  assert_sir_structure(
    r#"
fun badge(label: str) -> </> {
  return <span>{label}</span>;
}

fun main() {
  imu page ::= <div><badge label="new"><p>extra</p></badge></div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;

      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let texts: Vec<&str> = page
        .iter()
        .filter_map(|c| match c {
          UiCommand::Text(t) => Some(t.as_str()),
          _ => None,
        })
        .collect();

      assert!(
        texts.iter().any(|t| t.contains("new"))
          && texts.iter().any(|t| t.contains("extra")),
        "children dropped by slotless component: {texts:?}"
      );
    },
  );
}

#[test]
fn self_closing_slotted_component_renders_empty_slot() {
  assert_sir_structure(
    r#"
fun card(title: str) -> </> {
  return <div>
    <h2>{title}</h2>
    <slot />
  </div>;
}

fun main() {
  imu page ::= <div><card title="alone" /></div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;

      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let has_title = page
        .iter()
        .any(|c| matches!(c, UiCommand::Text(t) if t.contains("alone")));
      let stray_slot = page.iter().any(|c| {
        matches!(c, UiCommand::Element { tag, .. }
          if format!("{tag:?}").to_lowercase().contains("slot"))
      });

      assert!(has_title, "self-closing slotted card missing: {page:#?}");
      assert!(!stray_slot, "literal slot element leaked: {page:#?}");
    },
  );
}

#[test]
fn reactive_children_keep_bindings_through_the_slot() {
  // Children build in the parent's scope: a `mut` interpolation
  // inside them must arrive in the page template as a live text
  // binding, drained from the parent walk, carried through the
  // slot, and rebased twice (children-relative, then instance
  // splice position).
  assert_sir_structure(
    r#"
fun card() -> </> {
  return <div><slot /></div>;
}

fun main() {
  mut count := 0;

  imu page ::= <card><span>{count}</span></card>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;

      let (commands, bindings) = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template {
            commands, bindings, ..
          } => Some((commands, bindings)),
          _ => None,
        })
        .next_back()
        .expect("page template");

      assert!(
        !bindings.text.is_empty(),
        "reactive child lost its binding through the slot: {bindings:?}"
      );

      for (cmd_idx, _) in &bindings.text {
        assert!(
          matches!(commands.get(*cmd_idx), Some(UiCommand::Text(_))),
          "slot-carried binding mistargeted: {commands:#?}"
        );
      }
    },
  );
}

#[test]
fn event_on_component_tag_reports_instead_of_dropping() {
  use super::common::assert_execution_error;

  use zo_error::ErrorKind;

  assert_execution_error(
    r#"
fun card(title: str) -> </> {
  return <div><h2>{title}</h2></div>;
}

fun main() {
  mut count := 0;

  imu page ::= <div>
    <card title="hi" @click={fn() => count += 1} />
  </div>;

  #render page;
}"#,
    ErrorKind::EventOnComponent,
  );
}

#[test]
fn callback_prop_routes_event_to_parent_closure() {
  // `on_close={fn() => open = false}` lowers the closure to a
  // named handler; the component binds it to its fn-typed param
  // and wires `@click={on_close}` — the page's Event command must
  // dispatch to the parent's synthesized closure, not to the
  // literal name `on_close`.
  assert_sir_structure(
    r#"
fun card(title: str, on_close: Fn() -> unit) -> </> {
  return <div>
    <h2>{title}</h2>
    <button @click={on_close}>x</button>
  </div>;
}

fun main() {
  mut open := 1;

  imu page ::= <div>
    <card title="hi" on_close={fn() => open = 0} />
  </div>;

  #render page;
}"#,
    |sir| {
      use zo_ui_protocol::UiCommand;
      use zo_value::FunctionKind;

      let page = sir
        .iter()
        .filter_map(|i| match i {
          Insn::Template { commands, .. } => Some(commands),
          _ => None,
        })
        .next_back()
        .expect("page template");

      let handler = page
        .iter()
        .find_map(|c| match c {
          UiCommand::Event { handler, .. } => Some(handler.as_str()),
          _ => None,
        })
        .expect("event command missing from page");

      assert_ne!(
        handler, "on_close",
        "event must dispatch to the parent's closure, not the \
         parameter's own name"
      );

      // The handler must be a synthesized closure name (symbol ids
      // aren't resolvable here, but the `__closure_` prefix plus a
      // Closure-kind FunDef in SIR pin the chain).
      assert!(
        handler.starts_with("__closure_"),
        "handler must be a synthesized closure name, got `{handler}`"
      );

      let has_closure_def = sir.iter().any(|i| {
        matches!(
          i,
          Insn::FunDef {
            kind: FunctionKind::Closure { .. },
            ..
          }
        )
      });

      assert!(has_closure_def, "parent closure FunDef missing from SIR");
    },
  );
}
