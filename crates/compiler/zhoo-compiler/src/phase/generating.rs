use super::Process;

use zhoo_codegen::codegen;
use zhoo_session::session::Session;

use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Generating {}

impl Process for Generating {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("generating.");
    codegen::generate()?;
    Ok(())
  }
}

impl std::fmt::Display for Generating {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "generating")
  }
}
