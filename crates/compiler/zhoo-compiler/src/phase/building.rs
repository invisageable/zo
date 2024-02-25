use super::Process;

use zhoo_builder::builder;
use zhoo_session::session::Session;

use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Building {}

impl Process for Building {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("building.");
    builder::build()?;
    Ok(())
  }
}

impl std::fmt::Display for Building {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "building")
  }
}
