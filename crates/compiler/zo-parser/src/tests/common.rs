use crate::Parser;

use zo_error::ErrorKind;
use zo_reporter::collect_errors;
// use zo_span::Span;
use zo_token::Token;
use zo_tokenizer::Tokenizer;
use zo_tree::NodeValue;

pub(crate) fn assert_nodes_stream(
  source: &str,
  expected: &[(Token, Option<NodeValue>)],
) {
  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, source);
  let parsing = parser.parse();

  let actual = parsing
    .tree
    .nodes
    .iter()
    .enumerate()
    .map(|(i, node)| (node.token, parsing.tree.value(i as u32)))
    .collect::<Vec<_>>();

  assert_eq!(
    actual,
    expected,
    "parsing failed with errors: {:#?}.",
    collect_errors()
      .iter()
      .map(|error| (error.kind(), error.span()))
      .collect::<Vec<_>>()
  );
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
