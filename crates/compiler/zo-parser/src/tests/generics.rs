use crate::tests::common::assert_nodes_stream;

use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_generic_fun_single_param() {
  assert_nodes_stream(
    r#"
      fun identity<$T>(x: $T) -> $T { x }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 8))),
      (LAngle, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(21, 1))), // T
      (RAngle, None),
      (LParen, None),
      (Ident, Some(NodeValue::TextRange(24, 1))), // x
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // T
      (RParen, None),
      (Arrow, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(34, 1))), // T
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(38, 1))), // x
      (RBrace, None),
    ],
  );
}

#[test]
fn test_generic_fun_multi_param() {
  assert_nodes_stream(
    r#"
      fun swap<$A, $B>(a: $A, b: $B) -> $B { b }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LAngle, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(17, 1))), // A
      (Comma, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(21, 1))), // B
      (RAngle, None),
      (LParen, None),
      (Ident, Some(NodeValue::TextRange(24, 1))), // a
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // A
      (Comma, None),
      (Ident, Some(NodeValue::TextRange(31, 1))), // b
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(35, 1))), // B
      (RParen, None),
      (Arrow, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(42, 1))), // B
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(46, 1))), // b
      (RBrace, None),
    ],
  );
}

#[test]
fn test_generic_struct() {
  assert_nodes_stream(
    r#"
      struct Pair<$T> {
        first: $T,
        second: $T,
      }
    "#,
    &[
      (Struct, None),
      (Ident, Some(NodeValue::TextRange(14, 4))),
      (LAngle, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(20, 1))), // T
      (RAngle, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(33, 5))), // first
      (Colon, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(41, 1))), // T
      (Comma, None),
      (Ident, Some(NodeValue::TextRange(52, 6))), // second
      (Colon, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(61, 1))), // T
      (Comma, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_generic_enum() {
  assert_nodes_stream(
    r#"
      enum Option<$T> {
        Some($T),
        None,
      }
    "#,
    &[
      (Enum, None),
      (Ident, Some(NodeValue::TextRange(12, 6))),
      (LAngle, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(20, 1))), // T
      (RAngle, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(33, 4))), // Some
      (LParen, None),
      (Dollar, None),
      (Ident, Some(NodeValue::TextRange(39, 1))), // T
      (RParen, None),
      (Comma, None),
      (Ident, Some(NodeValue::TextRange(51, 4))), // None
      (Comma, None),
      (RBrace, None),
    ],
  );
}
