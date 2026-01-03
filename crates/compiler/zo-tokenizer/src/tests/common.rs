use crate::Tokenizer;

use zo_error::ErrorKind;
use zo_reporter::collect_errors;
use zo_token::Token;

pub(crate) fn assert_tokens_stream(source: &str, expected: &[(Token, &str)]) {
  let tokenizer = Tokenizer::new(source);
  let tokenization = tokenizer.tokenize();

  let actual = tokenization
    .tokens
    .kinds
    .iter()
    .enumerate()
    .map(|(i, &kind)| {
      let start = tokenization.tokens.starts[i] as usize;
      let length = tokenization.tokens.lengths[i] as usize;
      let lexeme = &source[start..start + length];

      (kind, lexeme)
    })
    .collect::<Vec<_>>();

  assert_eq!(
    actual, expected,
    "\n\nToken stream mismatch for source:\n'{}'\n\nExpected:\n{:#?}\n\nActual:\n{:#?}\n",
    source, expected, actual
  );
}

pub(crate) fn assert_error(source: &str, expected_error: ErrorKind) {
  let tokenizer = Tokenizer::new(source);

  tokenizer.tokenize();

  let errors = collect_errors();

  assert!(
    errors.iter().any(|e| e.kind() == expected_error),
    "Expected error {:?}, but got: {:?}",
    expected_error,
    errors.iter().map(|e| e.kind()).collect::<Vec<_>>()
  );
}

pub(crate) fn assert_errors(source: &str, expected_errors: &[ErrorKind]) {
  let tokenizer = Tokenizer::new(source);

  tokenizer.tokenize();

  let actual_errors = collect_errors()
    .iter()
    .map(|e| e.kind())
    .collect::<Vec<_>>();

  assert_eq!(
    actual_errors, expected_errors,
    "\n\nError kinds mismatch for source:\n'{}'\n\nExpected:\n{:#?}\n\nActual:\n{:#?}\n",
    source, expected_errors, actual_errors
  );
}
