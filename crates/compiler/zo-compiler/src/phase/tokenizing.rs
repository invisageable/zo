use super::{On, Process};

use zo_reporter::Result;
use zo_session::session::Session;

/// The lexical analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Tokenizing;
impl Process for Tokenizing {
  fn process(&self, _session: &mut Session, on: On) -> Result<On> {
    println!("phase:{self}");
    Ok(on)
  }
}

impl std::fmt::Display for Tokenizing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "tokenizing")
  }
}
