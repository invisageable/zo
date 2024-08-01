use super::Process;

use zo_reporter::Result;
use zo_session::session::Session;

/// The syntax analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Parsing;
impl Process for Parsing {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("phase:{self}");
    Ok(())
  }
}

impl std::fmt::Display for Parsing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "parsing")
  }
}
