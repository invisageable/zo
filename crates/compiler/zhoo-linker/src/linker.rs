use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Linker {}

pub fn link() -> Result<()> {
  Ok(())
}
