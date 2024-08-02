use super::{Event, Process};

use zo_reporter::Result;
use zo_session::session::Session;

/// The semantic analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Analyzing;
impl Process for Analyzing {
  fn process(&self, _session: &mut Session, event: Event) -> Result<Event> {
    println!("phase:{self}");
    Ok(event)
  }
}

impl std::fmt::Display for Analyzing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "analyzing")
  }
}
