use super::{Event, Process};

use zo_interpreter::interpreter;
use zo_reporter::{error, Result};
use zo_session::session::Session;

/// The interpretation phase.
#[derive(Clone, Copy, Debug)]
pub struct Interpreting;
impl Process for Interpreting {
  fn process(&self, session: &mut Session, event: Event) -> Result<Event> {
    if let Event::Ast(ast) = event {
      // todo — needs work.
      if session.settings.has_verbose() {
        println!("phase:{self} — {ast:?}\n");
      }

      return interpreter::interpret(session, &ast).and_then(Event::value);
    }

    Err(error::internal::expected_event(event))
  }
}

impl std::fmt::Display for Interpreting {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "interpreting")
  }
}
