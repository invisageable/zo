use super::{On, Process};

use zo_reporter::Result;
use zo_session::session::Session;

/// The interpretation phase.
#[derive(Clone, Copy, Debug)]
pub struct Interpreting;
impl Process for Interpreting {
  fn process(&self, _session: &mut Session, on: On) -> Result<On> {
    println!("phase:{self}");
    Ok(on)
  }
}

impl std::fmt::Display for Interpreting {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "interpreting")
  }
}
