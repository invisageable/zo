use super::token::group::Group;
use super::token::int::BaseInt;
use super::token::kw::Kw;
use super::token::op::Op;
use super::token::punctuation::Punctuation;
use super::token::{Token, TokenKind};
use super::tokenizer;

use zo_reader::reader;
use zo_session::session::Session;

use zo_core::interner::symbol::Symbol;
use zo_core::span::Span;

#[test]
fn tokenize_empty() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/empty.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens.len() == 0))
    .unwrap();
}

#[test]
fn tokenize_comments() {
  let mut session = Session::default();

  session.settings.input = "../zo-notes/samples/test/tokens/comments.zo".into();

  let source = reader::read_file(&mut session).unwrap();

  tokenizer::tokenize(&mut session, &source)
    .map(|tokens| assert!(tokens.len() == 0))
    .unwrap();
}
