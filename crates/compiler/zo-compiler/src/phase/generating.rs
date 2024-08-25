use super::{Event, Process};

use zo_codegen::codegen;
use zo_reporter::{error, Result};
use zo_session::session::Session;

/// The code generation phase.
#[derive(Clone, Copy, Debug)]
pub struct Generating;
impl Process for Generating {
  fn process(&self, session: &mut Session, event: Event) -> Result<Event> {
    if let Event::Ast(ast) = event {
      // todo — needs work.
      if session.settings.has_verbose() {
        println!("phase:{self} — {ast:?}\n");
      }

      return codegen::generate(session, &ast).and_then(Event::bytecode);
    }

    Err(error::internal::expected_event(event))
  }
}

impl std::fmt::Display for Generating {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "generating")
  }
}
