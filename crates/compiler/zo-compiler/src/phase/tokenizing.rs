use super::{On, Process};

use zo_reporter::Result;
use zo_session::session::Session;
use zo_tokenizer::tokenizer;

/// The lexical analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Tokenizing;
impl Process for Tokenizing {
  fn process(&self, session: &mut Session, on: On) -> Result<On> {
    if let On::Bytes(source) = on {
      println!("phase:{self} — {source:?}");
      return tokenizer::tokenize(session, &source).and_then(On::tokens);
    }

    panic!()
  }
}

impl std::fmt::Display for Tokenizing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "tokenizing")
  }
}
