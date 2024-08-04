use super::{Event, Process};

use zo_parser::parser;
use zo_reporter::Result;
use zo_session::session::Session;

/// The syntax analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Parsing;
impl Process for Parsing {
  fn process(&self, session: &mut Session, event: Event) -> Result<Event> {
    if let Event::Tokens(tokens) = event {
      println!("phase:{self} — {tokens:#?}");
      return parser::parse(session, &tokens).and_then(Event::ast);
    }

    panic!()
  }
}

impl std::fmt::Display for Parsing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "parsing")
  }
}
