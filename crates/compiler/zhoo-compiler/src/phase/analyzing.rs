use super::Process;

use zhoo_analyzer::analyzer;
use zhoo_session::session::Session;

use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Analyzing {}

impl Process for Analyzing {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("analyzing.");
    analyzer::analyze()?;
    Ok(())
  }
}

impl std::fmt::Display for Analyzing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "analyzing")
  }
}
