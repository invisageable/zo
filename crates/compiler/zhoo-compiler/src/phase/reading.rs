use super::Process;

use zhoo_reader::reader;
use zhoo_session::session::Session;

use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Reading {}

impl Process for Reading {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("reading.");
    reader::read()?;
    Ok(())
  }
}

impl std::fmt::Display for Reading {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "reading")
  }
}
