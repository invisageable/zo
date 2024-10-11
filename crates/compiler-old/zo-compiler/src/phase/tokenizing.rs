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
      // todo — needs work.
      if session.settings.has_verbose() {
        println!("phase:{self} — {source:?}\n");
      }

      // let mut tokens = Vec::with_capacity(0usize);
      // for (_, source) in sources {
      //   tokens.push(tokenizer::tokenize(session, &source)?);
      // }

      let tokens = tokenizer::tokenize(session, &source)?;

      return Event::tokens(tokens);
    }

    Err(error::internal::expected_event(event))
  }
}

impl std::fmt::Display for Tokenizing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "tokenizing")
  }
}
