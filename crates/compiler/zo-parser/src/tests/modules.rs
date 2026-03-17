use crate::tests::common::assert_nodes_stream;

use zo_token::Token::{ColonColon, Ident, Load, Pack, Semicolon};
use zo_tree::NodeValue;

#[test]
fn test_load_simple() {
  assert_nodes_stream(
    "load std::math;",
    &[
      (Load, None),
      (Ident, Some(NodeValue::TextRange(5, 3))), // "std"
      (ColonColon, None),
      (Ident, Some(NodeValue::TextRange(10, 4))), // "math"
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_load_nested_path() {
  assert_nodes_stream(
    "load std::num::ops;",
    &[
      (Load, None),
      (Ident, Some(NodeValue::TextRange(5, 3))), // "std"
      (ColonColon, None),
      (Ident, Some(NodeValue::TextRange(10, 3))), // "num"
      (ColonColon, None),
      (Ident, Some(NodeValue::TextRange(15, 3))), // "ops"
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_load_single_segment() {
  assert_nodes_stream(
    "load foo;",
    &[
      (Load, None),
      (Ident, Some(NodeValue::TextRange(5, 3))), // "foo"
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_pack_declaration() {
  assert_nodes_stream(
    "pack io;",
    &[
      (Pack, None),
      (Ident, Some(NodeValue::TextRange(5, 2))), // "io"
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_load_before_function() {
  assert_nodes_stream(
    r#"load foo::bar;

fun main() {
  bar()
}"#,
    &[
      (Load, None),
      (Ident, Some(NodeValue::TextRange(5, 3))), // "foo"
      (ColonColon, None),
      (Ident, Some(NodeValue::TextRange(10, 3))), // "bar"
      (Semicolon, None),
      (zo_token::Token::Fun, None),
      (Ident, Some(NodeValue::TextRange(20, 4))), // "main"
      (zo_token::Token::LParen, None),
      (zo_token::Token::RParen, None),
      (zo_token::Token::LBrace, None),
      (Ident, Some(NodeValue::TextRange(33, 3))), // "bar"
      (zo_token::Token::LParen, None),
      (zo_token::Token::RParen, None),
      (zo_token::Token::RBrace, None),
    ],
  );
}

#[test]
fn test_multiple_loads() {
  assert_nodes_stream(
    r#"load math::add;
load utils::format;"#,
    &[
      (Load, None),
      (Ident, Some(NodeValue::TextRange(5, 4))), // "math"
      (ColonColon, None),
      (Ident, Some(NodeValue::TextRange(11, 3))), // "add"
      (Semicolon, None),
      (Load, None),
      (Ident, Some(NodeValue::TextRange(21, 5))), // "utils"
      (ColonColon, None),
      (Ident, Some(NodeValue::TextRange(28, 6))), // "format"
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_pack_multiple() {
  assert_nodes_stream(
    r#"pack io;
pack math;"#,
    &[
      (Pack, None),
      (Ident, Some(NodeValue::TextRange(5, 2))), // "io"
      (Semicolon, None),
      (Pack, None),
      (Ident, Some(NodeValue::TextRange(14, 4))), // "math"
      (Semicolon, None),
    ],
  );
}
