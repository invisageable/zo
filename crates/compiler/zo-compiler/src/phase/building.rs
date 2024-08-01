use super::{On, Process};

use zo_reporter::Result;
use zo_session::session::Session;

/// The build phase.
#[derive(Clone, Copy, Debug)]
pub struct Building;
impl Process for Building {
  fn process(&self, _session: &mut Session, on: On) -> Result<On> {
    println!("phase:{self}");
    Ok(on)
  }
}

impl std::fmt::Display for Building {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "building")
  }
}
