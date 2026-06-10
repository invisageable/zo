//! ```sh
//! cargo test -p zo-tokenizer --lib tests::expression_templates
//! ```
//!
//! Value-position templates: a `<` right after `return` / `{` /
//! `;` / `=>` opens a tag — a comparison there would have no left
//! operand. The whitelist is deliberately narrow: `::=` is the one
//! template *binding* form, so `:=` and `=` never open templates.

use super::common::assert_tokens_stream;

use zo_token::Token;

#[test]
fn comparison_keeps_lt() {
  assert_tokens_stream(
    "a < b;",
    &[
      (Token::Ident, "a"),
      (Token::Lt, "<"),
      (Token::Ident, "b"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn loop_condition_keeps_lt() {
  assert_tokens_stream(
    "while i < n { }",
    &[
      (Token::While, "while"),
      (Token::Ident, "i"),
      (Token::Lt, "<"),
      (Token::Ident, "n"),
      (Token::LBrace, "{"),
      (Token::RBrace, "}"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn shift_untouched() {
  assert_tokens_stream(
    "x = a << 2;",
    &[
      (Token::Ident, "x"),
      (Token::Eq, "="),
      (Token::Ident, "a"),
      (Token::LShift, "<<"),
      (Token::Int, "2"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn colon_eq_never_opens_a_template() {
  // `::=` is the one template binding form — after `:=` a `<`
  // stays `Lt` and the program fails downstream as the invalid
  // zo it is.
  assert_tokens_stream(
    "imu x := <p;",
    &[
      (Token::Imu, "imu"),
      (Token::Ident, "x"),
      (Token::ColonEq, ":="),
      (Token::Lt, "<"),
      (Token::Ident, "p"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn eq_never_opens_a_template() {
  assert_tokens_stream(
    "x = <p;",
    &[
      (Token::Ident, "x"),
      (Token::Eq, "="),
      (Token::Lt, "<"),
      (Token::Ident, "p"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn return_opens_a_template() {
  assert_tokens_stream(
    "return <h1>x</h1>;",
    &[
      (Token::Return, "return"),
      (Token::LAngle, "<"),
      (Token::Ident, "h1"),
      (Token::RAngle, ">"),
      (Token::TemplateText, "x"),
      (Token::LAngle, "<"),
      (Token::Slash2, "/"),
      (Token::Ident, "h1"),
      (Token::RAngle, ">"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn tail_template_restores_code_mode_at_root_close() {
  // No `;` after the template — the root close itself must hand
  // the `}` and everything after it back to code scanning.
  assert_tokens_stream(
    "fun f() -> </> { <h1>x</h1> } imu y := 1;",
    &[
      (Token::Fun, "fun"),
      (Token::Ident, "f"),
      (Token::LParen, "("),
      (Token::RParen, ")"),
      (Token::Arrow, "->"),
      (Token::TemplateType, "</>"),
      (Token::LBrace, "{"),
      (Token::LAngle, "<"),
      (Token::Ident, "h1"),
      (Token::RAngle, ">"),
      (Token::TemplateText, "x"),
      (Token::LAngle, "<"),
      (Token::Slash2, "/"),
      (Token::Ident, "h1"),
      (Token::RAngle, ">"),
      (Token::RBrace, "}"),
      (Token::Imu, "imu"),
      (Token::Ident, "y"),
      (Token::ColonEq, ":="),
      (Token::Int, "1"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn tail_template_after_a_statement() {
  // The `;` of the preceding statement is a template opener, so
  // a tail template after ordinary statements works too.
  assert_tokens_stream(
    "imu n := 1; <p>x</p> }",
    &[
      (Token::Imu, "imu"),
      (Token::Ident, "n"),
      (Token::ColonEq, ":="),
      (Token::Int, "1"),
      (Token::Semicolon, ";"),
      (Token::LAngle, "<"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::TemplateText, "x"),
      (Token::LAngle, "<"),
      (Token::Slash2, "/"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::RBrace, "}"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn match_arm_opens_a_template() {
  assert_tokens_stream(
    "x => <p>a</p>,",
    &[
      (Token::Ident, "x"),
      (Token::FatArrow, "=>"),
      (Token::LAngle, "<"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::TemplateText, "a"),
      (Token::LAngle, "<"),
      (Token::Slash2, "/"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::Comma, ","),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn return_fragment_opens_and_closes() {
  assert_tokens_stream(
    "return <></>;",
    &[
      (Token::Return, "return"),
      (Token::TemplateFragmentStart, "<>"),
      (Token::TemplateFragmentEnd, "</>"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}

#[test]
fn template_assign_path_is_unchanged() {
  assert_tokens_stream(
    "imu v ::= <p>hi</p>;",
    &[
      (Token::Imu, "imu"),
      (Token::Ident, "v"),
      (Token::TemplateAssign, "::="),
      (Token::LAngle, "<"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::TemplateText, "hi"),
      (Token::LAngle, "<"),
      (Token::Slash2, "/"),
      (Token::Ident, "p"),
      (Token::RAngle, ">"),
      (Token::Semicolon, ";"),
      (Token::Eof, ""),
    ],
  );
}
