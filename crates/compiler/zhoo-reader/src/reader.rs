use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Reader {}

pub fn read() -> Result<()> {
  println!("read.");
  Ok(())
}
