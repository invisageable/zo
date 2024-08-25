use super::{Event, Process};

use zo_reader::reader;
use zo_reporter::{error, Result};
use zo_session::session::Session;

/// The reading phase.
#[derive(Clone, Copy, Debug)]
pub struct Reading;
impl Process for Reading {
  fn process(&self, session: &mut Session, event: Event) -> Result<Event> {
    if let Event::Path(pathname) = &event {
      // todo — needs work.
      if session.settings.has_verbose() {
        println!("phase:{self} — {pathname:?}\n");
      }

      if session.settings.is_interactive() {
        return reader::read_line(session).and_then(Event::bytes);
      } else {
        return reader::read(session, pathname).and_then(Event::bytes);
      }
    }

    Err(error::internal::expected_event(event))
  }
}

impl std::fmt::Display for Reading {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "reading")
  }
}
