use super::parser;

use zhoo_reader::reader;
use zhoo_session::session::Session;
use zhoo_tokenizer::tokenizer;

lazy_static::lazy_static! {
  pub static ref SESSION: std::sync::Mutex<Session> = std::sync::Mutex::new(
    Session::default()
  );
}

#[test]
fn parse_empty() {
  let mut session = &mut SESSION.lock().unwrap();
  let source = "".as_bytes();
  let tokens = tokenizer::tokenize(&mut session, source).unwrap();
  let program = parser::parse(&mut session, &tokens).unwrap();

  assert!(program.items.len() == 0);
}

#[test]
fn parse_atlas() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/bench/grammar.zo".into();

  let source = reader::read_file(&mut session).unwrap();
  let tokens = tokenizer::tokenize(&mut session, &source).unwrap();
  let program = parser::parse(&mut session, &tokens).unwrap();

  assert!(program.items.len() > 0);
}
