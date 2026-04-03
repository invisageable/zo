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
