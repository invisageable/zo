use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Codegen {}

pub fn generate() -> Result<()> {
  println!("generate.");
  Ok(())
}
