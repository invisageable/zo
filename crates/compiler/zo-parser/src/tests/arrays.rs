use crate::tests::common::assert_nodes_stream;
use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_chained_indexing_produces_two_lbrackets() {
  // grid[0][1] must parse as two indexing operations,
  // not as grid[0] followed by an array literal [1].
  assert_nodes_stream(
    r#"
      fun main() {
        x = grid[0][1];
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // x
      (Eq, None),
      // grid[0] — first index
      (Ident, Some(NodeValue::TextRange(32, 4))), // grid
      (LBracket, None),
      (Int, Some(NodeValue::Literal(0))), // 0
      (RBracket, None),
      // [1] — second index (chained, not literal)
      (LBracket, None),
      (Int, Some(NodeValue::Literal(1))), // 1
      (RBracket, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_array_index_after_rparen() {
  // f()[0] — indexing result of a function call.
  assert_nodes_stream(
    r#"
      fun main() {
        x = f()[0];
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // x
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(32, 1))), // f
      (LParen, None),
      (RParen, None),
      // [0] — indexing call result
      (LBracket, None),
      (Int, Some(NodeValue::Literal(0))),
      (RBracket, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
