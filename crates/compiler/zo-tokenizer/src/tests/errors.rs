//! ```sh
//! cargo test -p zo-tokenizer --lib tests::errors
//! ```

use crate::tests::common::{assert_error, assert_errors};

use zo_error::ErrorKind;

#[test]
fn test_unmatched_opening_paren() {
  assert_error("(hello", ErrorKind::UnmatchedOpeningDelimiter);
}

#[test]
fn test_unmatched_opening_brace() {
  assert_error("{hello", ErrorKind::UnmatchedOpeningDelimiter);
}

#[test]
fn test_unmatched_opening_bracket() {
  assert_error("[hello", ErrorKind::UnmatchedOpeningDelimiter);
}

#[test]
fn test_unmatched_closing_paren() {
  assert_error("hello)", ErrorKind::UnmatchedClosingDelimiter);
}

#[test]
fn test_unmatched_closing_brace() {
  assert_error("hello}", ErrorKind::UnmatchedClosingDelimiter);
}

#[test]
fn test_unmatched_closing_bracket() {
  assert_error("hello]", ErrorKind::UnmatchedClosingDelimiter);
}

#[test]
fn test_mismatched_paren_bracket() {
  assert_errors(
    "(hello]",
    &[
      ErrorKind::UnmatchedClosingDelimiter,
      ErrorKind::UnmatchedOpeningDelimiter,
    ],
  );
}

#[test]
fn test_mismatched_brace_paren() {
  assert_errors(
    "{hello)",
    &[
      ErrorKind::UnmatchedClosingDelimiter,
      ErrorKind::UnmatchedOpeningDelimiter,
    ],
  );
}

#[test]
fn test_mismatched_bracket_brace() {
  assert_errors(
    "[hello}",
    &[
      ErrorKind::UnmatchedClosingDelimiter,
      ErrorKind::UnmatchedOpeningDelimiter,
    ],
  );
}

#[test]
fn test_multiple_unmatched_opening() {
  assert_error("((hello) world", ErrorKind::UnmatchedOpeningDelimiter);
}

#[test]
fn test_multiple_unmatched_closing() {
  assert_errors(
    "hello))",
    &[
      ErrorKind::UnmatchedClosingDelimiter,
      ErrorKind::UnmatchedClosingDelimiter,
    ],
  );
}

#[test]
fn test_complex_mismatched() {
  assert_errors(
    "{{[hello}]",
    &[
      ErrorKind::MismatchedDelimiter,
      ErrorKind::UnmatchedOpeningDelimiter,
      ErrorKind::UnmatchedClosingDelimiter,
      ErrorKind::UnmatchedOpeningDelimiter,
    ],
  );
}

#[test]
fn test_multiple_delimiter_errors() {
  assert_errors(
    "(hello} world [foo)",
    &[
      ErrorKind::UnmatchedClosingDelimiter,
      ErrorKind::MismatchedDelimiter,
      ErrorKind::UnmatchedOpeningDelimiter,
    ],
  );
}

#[test]
fn test_unterminated_string_with_delimiter() {
  assert_errors(
    r#"("hello"#,
    &[
      ErrorKind::UnterminatedString,
      ErrorKind::UnmatchedOpeningDelimiter,
    ],
  );
}

#[test]
fn test_unterminated_block_comment_with_delimiter() {
  assert_errors(
    "(-* hello",
    &[
      ErrorKind::UnterminatedBlockComment,
      ErrorKind::UnmatchedOpeningDelimiter,
    ],
  );
}

#[test]
fn test_empty_char_literal() {
  assert_error("''", ErrorKind::EmptyCharLiteral);
}

#[test]
fn test_unterminated_char_literal() {
  assert_error("'a", ErrorKind::UnterminatedChar);
}

#[test]
fn test_unterminated_raw_string() {
  assert_error(r#"$"hello"#, ErrorKind::UnterminatedString);
}

#[test]
fn test_nested_delimiters_wrong_order() {
  assert_errors(
    "([)]",
    &[
      ErrorKind::MismatchedDelimiter,
      ErrorKind::UnmatchedOpeningDelimiter,
      ErrorKind::UnmatchedClosingDelimiter,
    ],
  );
}

#[test]
fn test_multiple_unterminated_strings() {
  assert_error("\"hello", ErrorKind::UnterminatedString);
}

#[test]
fn test_unterminated_bytes_literal() {
  assert_error("`hello", ErrorKind::UnterminatedBytes);
}
