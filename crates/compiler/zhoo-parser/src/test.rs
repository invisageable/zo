#![allow(dead_code)]

use super::parser;

use zhoo_reader::reader;
use zhoo_session::session::Session;
use zhoo_tokenizer::tokenizer;

#[test]
fn parse_empty() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/empty.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let program = parser::parse(&mut session, &tokens).unwrap();

  assert!(program.items.len() == 0);
}

#[test]
#[ignore]
fn parse_item_load() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/load.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let program = parser::parse(&mut session, &tokens).unwrap();

  assert!(program.items.len() > 0);
}

#[test]
#[ignore]
fn parse_item_pack() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/item/pack.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_val() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/item/val.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_ty_alias() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/item/ty_alias.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_ext() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/item/ext.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_abstract() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/item/abstract.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_enum() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/item/enum.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_struct() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/item/struct.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_apply() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/item/apply.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_item_fun() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/item/fun.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_stmt_imu() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/stmt/imu.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_stmt_mut() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/stmt/mut.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_lit() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/expr/lit.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_unop() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/expr/unop.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_binop() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/binop.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_assign() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/assign.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_assignop() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/assignop.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_array() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/array.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_tuple() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/tuple.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_array_access() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/array_access.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_tuple_access() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/tuple-access.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_block() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/block.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_fn() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/expr/fn.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_call() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/expr/call.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_return() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/return.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_if_else() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/if_else.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_when() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/expr/when.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_match() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/match.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_loop() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/expr/loop.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_while() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/while.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_for() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/ast/expr/for.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_break() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/break.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_continue() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/continue.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_struct_expr() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/struct_expr.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}

#[test]
#[ignore]
fn parse_expr_chaining() {
  let mut session = Session::default();

  session.settings.input =
    "../zhoo-notes/samples/test/ast/expr/chaining.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let _program = parser::parse(&mut session, &tokens).unwrap();

  assert!(true);
}
