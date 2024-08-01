use super::Process;

use zo_reporter::Result;
use zo_session::session::Session;

/// The reading phase.
#[derive(Clone, Copy, Debug)]
pub struct Reading;
impl Process for Reading {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("phase:{self}");
    Ok(())
  }
}

impl std::fmt::Display for Reading {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "reading")
  }
}
