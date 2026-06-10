//! ```sh
//! cargo test -p zo-parser --lib tests::expression_templates
//! ```
//!
//! Value-position templates: the tokenizer opens template mode for
//! a `<` after `return` / `{` / `;` / `=>`; the parser wraps the
//! named-tag root in the same synthetic fragment the `::=` and
//! `=:>` paths use, so `execute_template_fragment` sees one shape.

use crate::tests::common::assert_nodes_stream;

use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn return_template_wraps_in_synthetic_fragment() {
  assert_nodes_stream(
    "fun f() -> </> { return <p>x</p>; }",
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(4, 1))), // "f"
      (LParen, None),
      (RParen, None),
      (Arrow, None),
      (TemplateType, None),
      (LBrace, None),
      (Return, None),
      (TemplateFragmentStart, None),
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(25, 1))), // "p"
      (RAngle, None),
      (TemplateText, Some(NodeValue::TextRange(27, 1))), // "x"
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(30, 1))), // "p"
      (RAngle, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn tail_template_closes_at_rbrace() {
  // No `;` — the function body's `}` is the fragment boundary
  // and must land outside it, at code level.
  assert_nodes_stream(
    "fun f() -> </> { <p>x</p> }",
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(4, 1))), // "f"
      (LParen, None),
      (RParen, None),
      (Arrow, None),
      (TemplateType, None),
      (LBrace, None),
      (TemplateFragmentStart, None),
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(18, 1))), // "p"
      (RAngle, None),
      (TemplateText, Some(NodeValue::TextRange(20, 1))), // "x"
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(23, 1))), // "p"
      (RAngle, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn comparison_still_parses_as_operator() {
  // `<` with a left operand stays a comparison — the expr
  // buffer reorders `1 < 2` into postfix `Int Int Lt`.
  assert_nodes_stream(
    "fun f() { imu a := 1 < 2; }",
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(4, 1))), // "f"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(14, 1))), // "a"
      (ColonEq, None),
      (Int, Some(NodeValue::Literal(0))),
      (Int, Some(NodeValue::Literal(1))),
      (Lt, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
