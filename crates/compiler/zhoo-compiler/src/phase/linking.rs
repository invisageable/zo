use super::Process;

use zhoo_linker::linker;
use zhoo_session::session::Session;

use zo_core::Result;

#[derive(Debug)]
pub struct Linking {}

impl Process for Linking {
  fn process(&self, _session: &mut Session) -> Result<()> {
    println!("linking.");
    linker::link()?;
    Ok(())
  }
}

impl std::fmt::Display for Linking {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "linking")
  }
}
