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
      // x = -42 (postfix: 42, -)
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Int, Some(NodeValue::Literal(0))), // 42
      (UnaryMinus, None),                 // unary minus
      (Semicolon, None),
      // y = -x (postfix: x, -)
      (Ident, Some(NodeValue::TextRange(45, 1))), // "y"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(50, 1))), // "x"
      (UnaryMinus, None),                         // unary minus
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
      // a = !true (postfix: true, !)
      (Ident, Some(NodeValue::TextRange(28, 1))), // "a"
      (Eq, None),
      (True, None),
      (Bang, None), // unary not
      (Semicolon, None),
      // b = !flag (postfix: flag, !)
      (Ident, Some(NodeValue::TextRange(47, 1))), // "b"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(52, 4))), // "flag"
      (Bang, None),                               // unary not
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
      // result = x + -y (postfix: x, y, -, +)
      (Ident, Some(NodeValue::TextRange(28, 6))), // "result"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(37, 1))), // "x"
      (Ident, Some(NodeValue::TextRange(42, 1))), // "y"
      (UnaryMinus, None),                         // unary minus
      (Plus, None),                               // binary plus
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_unary_not_binds_looser_than_dot_call() {
  // `!x.foo()` must parse as `!(x.foo())`, not `(!x).foo()`.
  // Postfix order: `x foo . ( ) !` — the `Bang` lands at
  // the end of the expression, AFTER the method chain.
  // Regression guard for the parser precedence fix where
  // `Bang` used to drain before a trailing `Dot`.
  assert_nodes_stream(
    r#"
      fun main() {
        a = !x.foo();
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // a = !x.foo()  → postfix: `x foo . ( ) !`
      (Ident, Some(NodeValue::TextRange(28, 1))),
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(33, 1))),
      (Ident, Some(NodeValue::TextRange(35, 3))),
      (Dot, None),
      (LParen, None),
      (RParen, None),
      (Bang, None),
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
      // ptr = &value (postfix: value, &)
      (Ident, Some(NodeValue::TextRange(28, 3))), // "ptr"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(35, 5))), // "value"
      (Amp, None),                                // unary reference
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
      // value = *ptr (postfix: ptr, *)
      (Ident, Some(NodeValue::TextRange(28, 5))), // "value"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(37, 3))), // "ptr"
      (Star, None),                               // unary dereference
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
      // x = (-5) (postfix: 5, -)
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (LParen, None),
      (Int, Some(NodeValue::Literal(0))), // 5
      (UnaryMinus, None),                 // unary minus
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
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // x = !!flag (postfix: flag, !, !)
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(34, 4))), // "flag"
      (Bang, None),                               // inner not
      (Bang, None),                               // outer not
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_unary_not_on_function_call() {
  assert_nodes_stream(
    r#"
      fun main() {
        x = !f(0);
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // x = !f(0) — ! comes AFTER the complete call.
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(33, 1))), // "f"
      (LParen, None),
      (Int, Some(NodeValue::Literal(0))), // 0
      (RParen, None),
      (Bang, None), // unary not after call result
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
