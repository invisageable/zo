use crate::tests::common::assert_nodes_stream;

use zo_interner::Symbol;
use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_directive_run() {
  assert_nodes_stream(
    r#"
      #run foobar();
    "#,
    &[
      (Hash, None),
      (Ident, Some(NodeValue::Symbol(Symbol(25)))), // "run"
      (Ident, Some(NodeValue::Symbol(Symbol(26)))), // "foobar"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_directive_in_function() {
  assert_nodes_stream(
    r#"
      fun main() {
        #run foobar();
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::Symbol(Symbol(25)))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Hash, None),
      (Ident, Some(NodeValue::Symbol(Symbol(26)))), // "run"
      (Ident, Some(NodeValue::Symbol(Symbol(27)))), // "foobar"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_directive_with_expression() {
  assert_nodes_stream(
    r#"
      #inline 1 + 2 * 3;
    "#,
    &[
      (Hash, None),
      (Ident, Some(NodeValue::Symbol(Symbol(25)))), // "inline"
      (Int, Some(NodeValue::Literal(0))),           // 1
      (Int, Some(NodeValue::Literal(1))),           // 2
      (Int, Some(NodeValue::Literal(2))),           // 3
      (Star, None),                                 // *
      (Plus, None),                                 // +
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_directive_dom_with_template() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu view: </> ::= <>hello world</>;
        #dom view;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::Symbol(Symbol(25)))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::Symbol(Symbol(26)))), // "view"
      (TemplateType, None), // </> as a single token for type annotation
      (TemplateAssign, None), // ::= switches to template mode
      (TemplateFragmentStart, None), /* <> (now properly recognized in
                             * template mode) */
      (TemplateText, Some(NodeValue::Symbol(Symbol(27)))), /* "hello world"
                                                            * (interned) */
      (TemplateFragmentEnd, None), /* </> (properly recognized in template
                                    * mode) */
      (Semicolon, None),
      (Hash, None),
      (Ident, Some(NodeValue::Symbol(Symbol(28)))), // "dom"
      (Ident, Some(NodeValue::Symbol(Symbol(26)))), // "view" (reused symbol)
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
