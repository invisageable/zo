use super::{Event, Process};

use zo_analyzer::analyzer;
use zo_reporter::{error, Result};
use zo_session::session::Session;

/// The semantic analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Analyzing;
impl Process for Analyzing {
  fn process(&self, session: &mut Session, event: Event) -> Result<Event> {
    if let Event::Ast(ast) = event {
      println!("phase:{self} — {ast:?}");
      return analyzer::analyze(session, &ast).and_then(Event::ast);
    }

    Err(error::internal::expected_event(event))
  }
}

impl std::fmt::Display for Analyzing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "analyzing")
  }
}
