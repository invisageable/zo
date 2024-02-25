use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Interpreter {}

pub fn interpret() -> Result<()> {
  println!("interpret.");
  Ok(())
}
