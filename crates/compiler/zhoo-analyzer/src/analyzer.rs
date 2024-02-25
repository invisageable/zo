use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Analyzer {}

pub fn analyze() -> Result<()> {
  println!("analyze.");
  Ok(())
}
