use super::Process;

use zo_reporter::Result;
use zo_session::session::Session;

/// The code generation phase.
#[derive(Clone, Copy, Debug)]
pub struct Generating;
impl Process for Generating {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("phase:{self}");
    Ok(())
  }
}

impl std::fmt::Display for Generating {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "generating")
  }
}
