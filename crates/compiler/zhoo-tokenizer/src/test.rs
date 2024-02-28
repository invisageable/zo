use super::tokenizer;

use zhoo_session::session::Session;

lazy_static::lazy_static! {
  pub static ref SESSION: std::sync::Mutex<Session> = std::sync::Mutex::new(Session::default());
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

  let source = r#"
-- this is a line comments
-! this is a line doc comments

0 1 2 3 4 5 6 7 8 9 10 100 1000 10000 100_000 1_000_000

= + - * / % ^ !
() {} []
, . : ;

abstract apply  async  await break continue else  enum ext fn
for      fun    if     imu   load  loop     match me   mut pack
pub      return struct type  val   wasm     while
  "#
  .as_bytes();

  tokenizer::tokenize(&mut session, source)
    .map(|tokens| assert!(tokens.len() > 0))
    .unwrap();
}
