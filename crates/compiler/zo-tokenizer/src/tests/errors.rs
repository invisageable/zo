//! ```sh
//! cargo test -p zo-tokenizer --lib tests::errors
//! ```

use crate::tests::common::{assert_error, assert_errors, assert_tokens_stream};

use zo_error::ErrorKind;
use zo_token::Token;

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
  assert_error("(hello]", ErrorKind::MismatchedDelimiter);
}

#[test]
fn test_mismatched_brace_paren() {
  assert_error("{hello)", ErrorKind::MismatchedDelimiter);
}

#[test]
fn test_mismatched_bracket_brace() {
  assert_error("[hello}", ErrorKind::MismatchedDelimiter);
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
      ErrorKind::UnmatchedOpeningDelimiter,
      ErrorKind::MismatchedDelimiter,
    ],
  );
}

#[test]
fn test_multiple_delimiter_errors() {
  assert_errors(
    "(hello} world [foo)",
    &[
      ErrorKind::MismatchedDelimiter,
      ErrorKind::MismatchedDelimiter,
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

/// `scan_char` used to advance one byte past the opening
/// quote, leaving the cursor inside a multi-byte UTF-8
/// sequence and reporting a spurious `UnterminatedChar`
/// for every non-ASCII codepoint. Regression guard: the
/// tokenizer must emit exactly one `Token::Char` whose
/// lexeme spans the full `'X'` including both quotes.
///
/// Covers all four UTF-8 byte-lengths:
/// - 2 bytes: `¥`, `ç`, `þ`, `ÿ`, `Π`
/// - 3 bytes: `€`
/// - 4 bytes: `🎉`
///
/// `utf8_cp_len`'s lead-byte dispatch table needs every
/// byte-length exercised or a bad branch could hide.
#[test]
fn test_utf8_char_literal_tokenizes() {
  for ch in ["'¥'", "'ç'", "'þ'", "'ÿ'", "'Π'", "'€'", "'🎉'"] {
    assert_tokens_stream(ch, &[(Token::Char, ch), (Token::Eof, "")]);
  }
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
