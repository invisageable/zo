use super::Process;

use zo_reporter::Result;
use zo_session::session::Session;

/// The lexical analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Tokenizing;
impl Process for Tokenizing {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("phase:{self}");
    Ok(())
  }
}

impl std::fmt::Display for Tokenizing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "tokenizing")
  }
}
