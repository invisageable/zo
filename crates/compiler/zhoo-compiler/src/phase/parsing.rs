use super::Process;

use zhoo_parser::parser;
use zhoo_session::session::Session;

use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Parsing {}

impl Process for Parsing {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("parsing.");
    parser::parse()?;
    Ok(())
  }
}

impl std::fmt::Display for Parsing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "parsing")
  }
}
