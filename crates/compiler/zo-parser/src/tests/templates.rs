use crate::tests::common::assert_nodes_stream;

use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_template_fat_arrow_closure_body() {
  // `fn(t) =:> <li>{t}</li>` parses as a closure whose body
  // is a template element. The auto-fragment wrap mirrors
  // the `::=` binding form so `execute_template_fragment`
  // sees the same shape regardless of whether the template
  // came from a binding or a closure return.
  assert_nodes_stream(
    "fun main() { imu wrap := fn(t: str) =:> <li>{t}</li>; }",
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(4, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(17, 4))), // "wrap"
      (ColonEq, None),
      (Fn, None),
      (LParen, None),
      (Ident, Some(NodeValue::TextRange(28, 1))), // "t"
      (StrType, None),
      (RParen, None),
      (TemplateFatArrow, None),
      (TemplateFragmentStart, None),
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(41, 2))), // "li"
      (RAngle, None),
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(45, 1))), // "t"
      (RBrace, None),
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(50, 2))), // "li"
      (RAngle, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_template_fragment_simple() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu content ::= <>Hello World</>;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 7))), // "content"
      (TemplateAssign, None),
      (TemplateFragmentStart, None),
      (TemplateText, Some(NodeValue::TextRange(46, 11))), // "Hello World"
      (TemplateFragmentEnd, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_template_element_simple() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu view ::= <div>Hello</div>;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 4))), // "view"
      (TemplateAssign, None),
      (TemplateFragmentStart, None),
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(42, 3))), // "div"
      (RAngle, None),
      (TemplateText, Some(NodeValue::TextRange(46, 5))), // "Hello"
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(53, 3))), // "div"
      (RAngle, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_template_with_interpolation() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu greeting ::= <div>Hello {name}</div>;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 8))), // "greeting"
      (TemplateAssign, None),
      (TemplateFragmentStart, None),
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(46, 3))), // "div"
      (RAngle, None),
      (TemplateText, Some(NodeValue::TextRange(50, 6))), // "Hello "
      (LBrace, None),
      (Ident, Some(NodeValue::TextRange(57, 4))), // "name"
      (RBrace, None),
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(64, 3))), // "div"
      (RAngle, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_template_with_attributes() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu btn ::= <button class="primary">Click</button>;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 3))), // "btn"
      (TemplateAssign, None),
      (TemplateFragmentStart, None),
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(41, 6))), // "button"
      (Ident, Some(NodeValue::TextRange(48, 5))), // "class"
      (Eq, None),
      (String, Some(NodeValue::TextRange(54, 9))), // "primary"
      (RAngle, None),
      (TemplateText, Some(NodeValue::TextRange(64, 5))), // "Click"
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(71, 6))), // "button"
      (RAngle, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_template_self_closing() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu input ::= <input type="text" />;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 5))), // "input"
      (TemplateAssign, None),
      (TemplateFragmentStart, None),
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(43, 5))), // "input"
      // "type" is now Ident in template tag markup.
      (Ident, Some(NodeValue::TextRange(49, 4))), // "type"
      (Eq, None),
      (String, Some(NodeValue::TextRange(54, 6))), // "text"
      (Slash2, None),
      (RAngle, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_template_nested() {
  assert_nodes_stream(
    r#"
      fun main() {
        imu card ::= <div><h1>Title</h1><p>Text</p></div>;
      }
    "#,
    &[
      (Fun, None),
      (Ident, Some(NodeValue::TextRange(11, 4))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (Imu, None),
      (Ident, Some(NodeValue::TextRange(32, 4))), // "card"
      (TemplateAssign, None),
      (TemplateFragmentStart, None),
      // Outer div
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(42, 3))), // "div"
      (RAngle, None),
      // h1
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(47, 2))), // "h1"
      (RAngle, None),
      (TemplateText, Some(NodeValue::TextRange(50, 5))), // "Title"
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(57, 2))), // "h1"
      (RAngle, None),
      // p
      (LAngle, None),
      (Ident, Some(NodeValue::TextRange(61, 1))), // "p"
      (RAngle, None),
      (TemplateText, Some(NodeValue::TextRange(63, 4))), // "Text"
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(69, 1))), // "p"
      (RAngle, None),
      // Closing div
      (LAngle, None),
      (Slash2, None),
      (Ident, Some(NodeValue::TextRange(73, 3))), // "div"
      (RAngle, None),
      (Semicolon, None),
      (RBrace, None),
    ],
  );
}
