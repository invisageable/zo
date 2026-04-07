use crate::tests::common::assert_nodes_stream;

use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_style_block_simple() {
  assert_nodes_stream(
    "$: { p { color: cyan; } }",
    &[
      (Dollar, None),
      (Colon, None),
      (LBrace, None),
      // "p"
      (Ident, Some(NodeValue::TextRange(5, 1))),
      (LBrace, None),
      // "color"
      (Ident, Some(NodeValue::TextRange(9, 5))),
      (Colon, None),
      // "cyan"
      (StyleValue, Some(NodeValue::TextRange(16, 4))),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_style_block_multiple_props() {
  assert_nodes_stream(
    "$: { .title { fw: 800; ta: center; } }",
    &[
      (Dollar, None),
      (Colon, None),
      (LBrace, None),
      (Dot, None),
      // "title"
      (Ident, Some(NodeValue::TextRange(6, 5))),
      (LBrace, None),
      // "fw"
      (Ident, Some(NodeValue::TextRange(14, 2))),
      (Colon, None),
      // "800"
      (StyleValue, Some(NodeValue::TextRange(18, 3))),
      (Semicolon, None),
      // "ta"
      (Ident, Some(NodeValue::TextRange(23, 2))),
      (Colon, None),
      // "center"
      (StyleValue, Some(NodeValue::TextRange(27, 6))),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_style_global_with_pub() {
  assert_nodes_stream(
    "pub $: { html body { w: 100%; } }",
    &[
      (Pub, None),
      (Dollar, None),
      (Colon, None),
      (LBrace, None),
      // "html"
      (Ident, Some(NodeValue::TextRange(9, 4))),
      // "body"
      (Ident, Some(NodeValue::TextRange(14, 4))),
      (LBrace, None),
      // "w"
      (Ident, Some(NodeValue::TextRange(21, 1))),
      (Colon, None),
      // "100%"
      (StyleValue, Some(NodeValue::TextRange(24, 4))),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_style_before_function() {
  assert_nodes_stream(
    "$: { p { color: cyan; } } fun main() {}",
    &[
      (Dollar, None),
      (Colon, None),
      (LBrace, None),
      // "p"
      (Ident, Some(NodeValue::TextRange(5, 1))),
      (LBrace, None),
      // "color"
      (Ident, Some(NodeValue::TextRange(9, 5))),
      (Colon, None),
      // "cyan"
      (StyleValue, Some(NodeValue::TextRange(16, 4))),
      (Semicolon, None),
      (RBrace, None),
      (RBrace, None),
      (Fun, None),
      // "main"
      (Ident, Some(NodeValue::TextRange(30, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (RBrace, None),
    ],
  );
}
