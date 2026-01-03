use crate::tests::common::assert_nodes_stream;
use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_compound_assignments() {
  assert_nodes_stream(
    r#"
      fun main() {
        x += 1;
        y -= 2;
        z *= 3;
        w /= 4;
        a %= 5;
        b &= 6;
        c |= 7;
        d ^= 8;
        e <<= 1;
        f >>= 2;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // x += 1
      (Ident, Some(NodeValue::TextRange(28, 1))), // "x"
      (PlusEq, None),
      (Int, Some(NodeValue::Literal(0))), // 1
      (Semicolon, None),
      // y -= 2
      (Ident, Some(NodeValue::TextRange(44, 1))), // "y"
      (MinusEq, None),
      (Int, Some(NodeValue::Literal(1))), // 2
      (Semicolon, None),
      // z *= 3
      (Ident, Some(NodeValue::TextRange(60, 1))), // "z"
      (StarEq, None),
      (Int, Some(NodeValue::Literal(2))), // 3
      (Semicolon, None),
      // w /= 4
      (Ident, Some(NodeValue::TextRange(76, 1))), // "w"
      (SlashEq, None),
      (Int, Some(NodeValue::Literal(3))), // 4
      (Semicolon, None),
      // a %= 5
      (Ident, Some(NodeValue::TextRange(92, 1))), // "a"
      (PercentEq, None),
      (Int, Some(NodeValue::Literal(4))), // 5
      (Semicolon, None),
      // b &= 6
      (Ident, Some(NodeValue::TextRange(108, 1))), // "b"
      (AmpEq, None),
      (Int, Some(NodeValue::Literal(5))), // 6
      (Semicolon, None),
      // c |= 7
      (Ident, Some(NodeValue::TextRange(124, 1))), // "c"
      (PipeEq, None),
      (Int, Some(NodeValue::Literal(6))), // 7
      (Semicolon, None),
      // d ^= 8
      (Ident, Some(NodeValue::TextRange(140, 1))), // "d"
      (CaretEq, None),
      (Int, Some(NodeValue::Literal(7))), // 8
      (Semicolon, None),
      // e <<= 1
      (Ident, Some(NodeValue::TextRange(156, 1))), // "e"
      (LShiftEq, None),
      (Int, Some(NodeValue::Literal(8))), // 1
      (Semicolon, None),
      // f >>= 2
      (Ident, Some(NodeValue::TextRange(173, 1))), // "f"
      (RShiftEq, None),
      (Int, Some(NodeValue::Literal(9))), // 2
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_compound_assignment_with_expression() {
  assert_nodes_stream(
    r#"
      fun main() {
        sum += x * 2;
        product *= y + 1;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // sum += x * 2
      (Ident, Some(NodeValue::TextRange(28, 3))), // "sum"
      (PlusEq, None),
      (Ident, Some(NodeValue::TextRange(35, 1))), // "x"
      (Int, Some(NodeValue::Literal(0))),         // 2
      (Star, None),
      (Semicolon, None),
      // product *= y + 1
      (Ident, Some(NodeValue::TextRange(50, 7))), // "product"
      (StarEq, None),
      (Ident, Some(NodeValue::TextRange(61, 1))), // "y"
      (Int, Some(NodeValue::Literal(1))),         // 1
      (Plus, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
