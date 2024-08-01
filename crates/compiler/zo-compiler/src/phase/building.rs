use super::Process;

use zo_reporter::Result;
use zo_session::session::Session;

/// The build phase.
#[derive(Clone, Copy, Debug)]
pub struct Building;
impl Process for Building {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("phase:{self}");
    Ok(())
  }
}

impl std::fmt::Display for Building {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "building")
  }
}
