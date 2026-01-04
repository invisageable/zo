use crate::Parser;

use zo_error::ErrorKind;
use zo_reporter::collect_errors;
// use zo_span::Span;
use zo_token::Token;
use zo_tokenizer::Tokenizer;
use zo_tree::NodeValue;

/// Compare NodeValue by variant only (not exact value for Symbol/TextRange)
fn node_values_match(
  actual: &Option<NodeValue>,
  expected: &Option<NodeValue>,
) -> bool {
  match (actual, expected) {
    (None, None) => true,
    (Some(NodeValue::Literal(a)), Some(NodeValue::Literal(e))) => a == e,
    // Symbol in actual matches either Symbol or TextRange in expected
    (Some(NodeValue::Symbol(_)), Some(NodeValue::Symbol(_))) => true,
    (Some(NodeValue::Symbol(_)), Some(NodeValue::TextRange(_, _))) => true,
    (Some(NodeValue::TextRange(_, _)), Some(NodeValue::TextRange(_, _))) => {
      true
    }
    (Some(NodeValue::TextRange(_, _)), Some(NodeValue::Symbol(_))) => true,
    _ => false,
  }
}

pub(crate) fn assert_nodes_stream(
  source: &str,
  expected: &[(Token, Option<NodeValue>)],
) {
  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let actual: Vec<_> = parsing
    .tree
    .nodes
    .iter()
    .enumerate()
    .map(|(i, node)| (node.token, parsing.tree.value(i as u32)))
    .collect();

  // Check length first
  assert_eq!(
    actual.len(),
    expected.len(),
    "Node count mismatch. Expected {} nodes, got {}.\n\nActual:\n{:#?}\n\nExpected:\n{:#?}\n\nErrors: {:#?}",
    expected.len(),
    actual.len(),
    actual,
    expected,
    collect_errors()
      .iter()
      .map(|error| (error.kind(), error.span()))
      .collect::<Vec<_>>()
  );

  // Check each node
  for (i, ((actual_token, actual_value), (expected_token, expected_value))) in
    actual.iter().zip(expected.iter()).enumerate()
  {
    assert_eq!(
      actual_token, expected_token,
      "Token mismatch at index {}.\n\nActual: {:?}\nExpected: {:?}",
      i, actual_token, expected_token
    );
    assert!(
      node_values_match(actual_value, expected_value),
      "NodeValue mismatch at index {}.\n\nActual: {:?}\nExpected: {:?}",
      i,
      actual_value,
      expected_value
    );
  }
}

pub(crate) fn assert_error(source: &str, expected_error: ErrorKind) {
  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let _parsing = parser.parse();

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == expected_error),
    "Expected error {:?}, but got: {:?}",
    expected_error,
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

// pub(crate) fn assert_errors(source: &str, expected_errors: &[ErrorKind]) {
//   let tokenizer = Tokenizer::new(source);
//   let tokenization = tokenizer.tokenize();

//   let parser = Parser::new(&tokenization, source);
//   let _parsing = parser.parse();

//   let actual_errors = collect_errors()
//     .iter()
//     .map(|error| error.kind())
//     .collect::<Vec<_>>();

//   assert_eq!(
//     actual_errors, expected_errors,
//     "\n\nError kinds mismatch for
// source:\n'{}'\n\nExpected:\n{:#?}\n\nActual:\n{:#?}\n",     source,
// expected_errors, actual_errors   );
// }
