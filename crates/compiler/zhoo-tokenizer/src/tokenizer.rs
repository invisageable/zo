use serde_derive::{Deserialize, Serialize};

use zo_core::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct Tokenizer {}

pub fn tokenize() -> Result<()> {
  println!("tokenize.");
  Ok(())
}
