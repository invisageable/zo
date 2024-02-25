use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Builder {}

pub fn build() -> Result<()> {
  println!("build.");
  Ok(())
}
