use crate::tests::common::assert_nodes_stream;
use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_unary_negation() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = -42;
        y = -x;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // x = -42
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Minus, None),                      // unary minus
      (Int, Some(NodeValue::Literal(0))), // 42
      (Semicolon, None),
      // y = -x
      (Ident, Some(NodeValue::TextRange(45, 1))), // "y"
      (Eq, None),
      (Minus, None),                              // unary minus
      (Ident, Some(NodeValue::TextRange(50, 1))), // "x"
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_unary_not() {
  assert_nodes_stream(
    r#"
      fun main() {
        a = !true;
        b = !flag;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // a = !true
      (Ident, Some(NodeValue::TextRange(28, 1))), // "a"
      (Eq, None),
      (Bang, None), // unary not
      (True, None),
      (Semicolon, None),
      // b = !flag
      (Ident, Some(NodeValue::TextRange(47, 1))), // "b"
      (Eq, None),
      (Bang, None),                               // unary not
      (Ident, Some(NodeValue::TextRange(52, 4))), // "flag"
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_unary_in_expression() {
  assert_nodes_stream(
    r#"
      fun main() {
        result = x + -y;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 6))), // "result"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(37, 1))), // "x"
      (Minus, None),                              // unary minus for -y
      (Ident, Some(NodeValue::TextRange(42, 1))), // "y"
      (Plus, None),                               // binary plus
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_unary_reference() {
  assert_nodes_stream(
    r#"
      fun main() {
        ptr = &value;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 3))), // "ptr"
      (Eq, None),
      (Amp, None),                                // unary reference
      (Ident, Some(NodeValue::TextRange(35, 5))), // "value"
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_unary_dereference() {
  assert_nodes_stream(
    r#"
      fun main() {
        value = *ptr;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 5))), // "value"
      (Eq, None),
      (Star, None), // unary dereference
      (Ident, Some(NodeValue::TextRange(37, 3))), // "ptr"
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_unary_in_parens() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = (-5);
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (LParen, None),
      (Minus, None),                      // unary minus
      (Int, Some(NodeValue::Literal(0))), // 5
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_multiple_unary() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = !!flag;
        y = -(-value);
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // x = !!flag
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Bang, None),                               // first not
      (Bang, None),                               // second not
      (Ident, Some(NodeValue::TextRange(34, 4))), // "flag"
      (Semicolon, None),
      // y = -(-value) (double negation with parens to avoid comment)
      (Ident, Some(NodeValue::TextRange(48, 1))), // "y"
      (Eq, None),
      (Minus, None), // outer negation
      (LParen, None),
      (Minus, None),                              // inner negation
      (Ident, Some(NodeValue::TextRange(55, 5))), // "value"
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
