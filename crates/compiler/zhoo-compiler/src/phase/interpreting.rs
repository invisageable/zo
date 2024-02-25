use super::Process;

use zhoo_interpreter::interpreter;
use zhoo_session::session::Session;

use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Interpreting {}

impl Process for Interpreting {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("interpreting.");
    interpreter::interpret()?;
    Ok(())
  }
}

impl std::fmt::Display for Interpreting {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "interpreting")
  }
}
