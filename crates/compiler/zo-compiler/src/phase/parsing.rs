use super::{On, Process};

use zo_reporter::Result;
use zo_session::session::Session;

/// The syntax analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Parsing;
impl Process for Parsing {
  fn process(&self, _session: &mut Session, on: On) -> Result<On> {
    println!("phase:{self}");
    Ok(on)
  }
}

impl std::fmt::Display for Parsing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "parsing")
  }
}
