use super::tokenizer;

use zhoo_reader::reader;
use zhoo_session::session::Session;

#[test]
fn tokenize_empty() {
  let mut session = Session::default();
  let source = "".as_bytes();

  tokenizer::tokenize(&mut session, source)
    .map(|tokens| assert!(tokens.len() == 0))
    .unwrap();
}

#[test]
fn tokenize_atlas() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/bench/atlas.tks".into();

  let source = reader::read_file(&mut session).unwrap();

  tokenizer::tokenize(&mut session, &source)
    // todo(ivs) — compare by tokens.
    .map(|tokens| assert!(tokens.len() > 0))
    .unwrap();
}
