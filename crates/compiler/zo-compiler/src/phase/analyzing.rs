use super::{On, Process};

use zo_reporter::Result;
use zo_session::session::Session;

/// The semantic analysis phase.
#[derive(Clone, Copy, Debug)]
pub struct Analyzing;
impl Process for Analyzing {
  fn process(&self, _session: &mut Session, on: On) -> Result<On> {
    println!("phase:{self}");
    Ok(on)
  }
}

impl std::fmt::Display for Analyzing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "analyzing")
  }
}
