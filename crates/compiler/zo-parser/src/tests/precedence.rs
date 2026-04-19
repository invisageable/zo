//! Precedence / paren-grouping tree-shape tests — regressions
//! for issue H (parser's `(` in expression context used to
//! flush the outer op stack prematurely, producing the wrong
//! postorder for `EXPR OP (PAREN) HIGH_PREC_OP RHS`).

use crate::tests::common::assert_nodes_stream;
use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_paren_group_after_low_prec_before_high_prec() {
  // `3 + (4 - 3) / 2` must parse as `3 + ((4-3)/2)`:
  //   postorder → 3, (, 4, 3, -, ), 2, /, +
  // OLD parser drained `+` at `(` and emitted it before
  // the group, so `/` ended up applying to `(3+group)`.
  assert_nodes_stream(
    r#"
      fun main() {
        imu a: int = 3 + (4 - 3) / 2;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 1))),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Int, Some(NodeValue::Literal(0))),
      (LParen, None),
      (Int, Some(NodeValue::Literal(1))),
      (Int, Some(NodeValue::Literal(2))),
      (Minus, None),
      (RParen, None),
      (Int, Some(NodeValue::Literal(3))),
      (Slash, None),
      (Plus, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_paren_group_with_ident_lhs() {
  // `x + (1)` — the parser fix emits `x` adjacent to `(`.
  // That adjacency used to mislead the executor's call
  // detection; tree shape alone is the parser's
  // responsibility.
  assert_nodes_stream(
    r#"
      fun main() {
        mut x: int = 3;
        imu a: int = x + (1);
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // mut x: int = 3;
      (Mut, None),
      (Ident, Some(NodeValue::TextRange(32, 1))),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Int, Some(NodeValue::Literal(0))),
      (Semicolon, None),
      // imu a: int = x + (1);
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(56, 1))),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(67, 1))),
      (LParen, None),
      (Int, Some(NodeValue::Literal(1))),
      (RParen, None),
      (Plus, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_paren_group_at_expression_start() {
  // `(2 + 3) * 4` — group at start, still emits outer
  // ops in postorder position.
  assert_nodes_stream(
    r#"
      fun main() {
        imu a: int = (2 + 3) * 4;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 1))),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (LParen, None),
      (Int, Some(NodeValue::Literal(0))),
      (Int, Some(NodeValue::Literal(1))),
      (Plus, None),
      (RParen, None),
      (Int, Some(NodeValue::Literal(2))),
      (Star, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_paren_group_logical_short_circuit_fallback() {
  // `x || (1)` — `||` in op_stack forces the old
  // full-flush branch so the executor can see the
  // logical op BEFORE the RHS subexpression.
  // Postorder shape: `x, ||, (, 1, )` — NOT `x, (, 1, ), ||`.
  assert_nodes_stream(
    r#"
      fun main() {
        mut x: bool = true;
        imu a: bool = x || (1 == 1);
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // mut x: bool = true;
      (Mut, None),
      (Ident, Some(NodeValue::TextRange(32, 1))),
      (BoolType, None),
      (Colon, None),
      (Eq, None),
      (True, None),
      (Semicolon, None),
      // imu a: bool = x || (1 == 1);
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(61, 1))),
      (BoolType, None),
      (Colon, None),
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(75, 1))),
      (PipePipe, None),
      (LParen, None),
      (Int, Some(NodeValue::Literal(0))),
      (Int, Some(NodeValue::Literal(1))),
      (EqEq, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_binary_search_midpoint_shape() {
  // `low + (high - low) / 2` — the real expression from
  // binary-search.zo that surfaced issue H. Postorder:
  //   low, (, high, low, -, ), 2, /, +
  assert_nodes_stream(
    r#"
      fun main() {
        mut low: int = 0;
        mut high: int = 4;
        imu mid: int = low + (high - low) / 2;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))),
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      // mut low: int = 0;
      (Mut, None),
      (Ident, Some(NodeValue::TextRange(32, 3))),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Int, Some(NodeValue::Literal(0))),
      (Semicolon, None),
      // mut high: int = 4;
      (Mut, None),
      (Ident, Some(NodeValue::TextRange(58, 4))),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Int, Some(NodeValue::Literal(1))),
      (Semicolon, None),
      // imu mid: int = low + (high - low) / 2;
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(85, 3))),
      (IntType, None),
      (Colon, None),
      (Eq, None),
      (Ident, Some(NodeValue::TextRange(96, 3))),
      (LParen, None),
      (Ident, Some(NodeValue::TextRange(103, 4))),
      (Ident, Some(NodeValue::TextRange(110, 3))),
      (Minus, None),
      (RParen, None),
      (Int, Some(NodeValue::Literal(2))),
      (Slash, None),
      (Plus, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
