//! `%%` attribute parser tests. Cover the three field
//! shapes (parameterless tag, call-style, key/value) and
//! the multi-field comma-separated form with optional
//! trailing comma. Each emits one `Attribute` node whose
//! children are the field tokens + Comma separators +
//! the closing Dot.

use crate::tests::common::assert_nodes_stream;

use zo_interner::Symbol;
use zo_token::Token::*;
use zo_tree::NodeValue;

#[test]
fn test_attribute_parameterless_tag() {
  // `%% inline.` — single name + Dot terminator.
  assert_nodes_stream(
    r#"
      %% inline.
      pub ffi noop();
    "#,
    &[
      (Attribute, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "inline"
      (Dot, None),
      (Pub, None),
      (Ffi, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "noop"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_attribute_call_style_ident() {
  // `%% cfg(linux).` — one ident-arg field.
  assert_nodes_stream(
    r#"
      %% cfg(linux).
      fun main() {}
    "#,
    &[
      (Attribute, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "cfg"
      (LParen, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "linux"
      (RParen, None),
      (Dot, None),
      (Fun, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "main"
      (LParen, None),
      (RParen, None),
      (LBrace, None),
      (RBrace, None),
    ],
  );
}

#[test]
fn test_attribute_key_value_string() {
  // `%% link_name = "InitWindow".` — key=string.
  assert_nodes_stream(
    r#"
      %% link_name = "InitWindow".
      pub ffi init_window();
    "#,
    &[
      (Attribute, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "link_name"
      (Eq, None),
      (String, Some(NodeValue::Symbol(Symbol(0)))), // "InitWindow"
      (Dot, None),
      (Pub, None),
      (Ffi, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "init_window"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_attribute_multi_field_with_trailing_comma() {
  // `%% link_name = "X", inline, abi = "C", .` — three
  // fields of mixed shapes + trailing comma. The Dot is
  // mandatory; the trailing comma before it is optional
  // sugar.
  assert_nodes_stream(
    r#"
      %%
        link_name = "InitWindow",
        inline,
        abi = "C",
      .
      pub ffi init_window();
    "#,
    &[
      (Attribute, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "link_name"
      (Eq, None),
      (String, Some(NodeValue::Symbol(Symbol(0)))), // "InitWindow"
      (Comma, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "inline"
      (Comma, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "abi"
      (Eq, None),
      (String, Some(NodeValue::Symbol(Symbol(0)))), // "C"
      (Comma, None),
      (Dot, None),
      (Pub, None),
      (Ffi, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "init_window"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_attribute_multi_field_no_trailing_comma() {
  // `%% link_name = "X", inline.` — same shape without
  // the trailing comma before Dot.
  assert_nodes_stream(
    r#"
      %% link_name = "X", inline.
      pub ffi foo();
    "#,
    &[
      (Attribute, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "link_name"
      (Eq, None),
      (String, Some(NodeValue::Symbol(Symbol(0)))), // "X"
      (Comma, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "inline"
      (Dot, None),
      (Pub, None),
      (Ffi, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "foo"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
    ],
  );
}

#[test]
fn test_attribute_stack_above_one_item() {
  // Two consecutive `%%` blocks both attach to the next
  // item — common shape for `%% link_name = "X".\n%%
  // deprecated.\npub ffi foo()`. Each block emits its
  // own Attribute node; the executor merges them into
  // one buffered set before the FunDef consumes.
  assert_nodes_stream(
    r#"
      %% link_name = "X".
      %% inline.
      pub ffi foo();
    "#,
    &[
      (Attribute, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "link_name"
      (Eq, None),
      (String, Some(NodeValue::Symbol(Symbol(0)))), // "X"
      (Dot, None),
      (Attribute, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "inline"
      (Dot, None),
      (Pub, None),
      (Ffi, None),
      (Ident, Some(NodeValue::Symbol(Symbol(0)))), // "foo"
      (LParen, None),
      (RParen, None),
      (Semicolon, None),
    ],
  );
}
