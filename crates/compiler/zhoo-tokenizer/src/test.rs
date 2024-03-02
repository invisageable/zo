use super::tokenizer;

use zhoo_reader::reader;
use zhoo_session::session::Session;

lazy_static::lazy_static! {
  pub static ref SESSION: std::sync::Mutex<Session> = std::sync::Mutex::new(
    Session::default()
  );
}

#[test]
fn tokenize_empty() {
  let mut session = &mut SESSION.lock().unwrap();
  let source = "".as_bytes();

  tokenizer::tokenize(&mut session, source)
    .map(|tokens| assert!(tokens.len() == 0))
    .unwrap();
}

#[test]
fn tokenize_atlas() {
  let mut session = &mut SESSION.lock().unwrap();

  session.settings.input = "../zhoo-notes/samples/bench/atlas.tks".into();

  let source = reader::read_file(&mut session).unwrap();

  tokenizer::tokenize(&mut session, &source)
    // todo(ivs) — compare by tokens.
    .map(|tokens| assert!(tokens.len() > 0))
    .unwrap();
}
