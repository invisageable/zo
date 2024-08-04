use super::{Event, Process};

use zo_reporter::{error, Result};
use zo_session::session::Session;
use zo_tokenizer::tokenizer;

/// The lexical analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Tokenizing;
impl Process for Tokenizing {
  fn process(&self, session: &mut Session, event: Event) -> Result<Event> {
    if let Event::Bytes(source) = event {
      println!("phase:{self} — {source:?}");
      return tokenizer::tokenize(session, &source).and_then(Event::tokens);
    }

    Err(error::internal::expected_event(event))
  }
}

impl std::fmt::Display for Tokenizing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "tokenizing")
  }
}
