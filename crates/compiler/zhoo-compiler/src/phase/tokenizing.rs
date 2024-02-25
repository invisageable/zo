use super::Process;

use zhoo_session::session::Session;
use zhoo_tokenizer::tokenizer;

use zo_core::Result;

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tokenizing {}

impl Process for Tokenizing {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("tokenizing.");
    tokenizer::tokenize()?;
    Ok(())
  }
}

impl std::fmt::Display for Tokenizing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "tokenizing")
  }
}
