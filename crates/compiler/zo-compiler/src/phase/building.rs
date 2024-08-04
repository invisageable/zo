use super::{Event, Process};

use zo_builder::builder;
use zo_reporter::{error, Result};
use zo_session::session::Session;

/// The build phase.
#[derive(Clone, Copy, Debug)]
pub struct Building;
impl Process for Building {
  fn process(&self, session: &mut Session, event: Event) -> Result<Event> {
    if let Event::Bytecode(bytecode) = event {
      println!("phase:{self} — {bytecode:?}");
      return builder::build(session, &bytecode).and_then(Event::output);
    }

    Err(error::internal::expected_event(event))
  }
}

impl std::fmt::Display for Building {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "building")
  }
}
