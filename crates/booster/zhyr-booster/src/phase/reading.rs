use super::Process;

use zhyr_reader::reader;

use zhoo_session::session::Session;

use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Clone, Debug)]
pub struct Reading {
  pub rx: Sender<Vec<std::path::PathBuf>>,
}

impl Process for Reading {
  fn process(&self, session: &mut Session) -> Result<()> {
    reader::read(session).and_then(|paths| self.rx.send(paths))
  }
}

impl std::fmt::Display for Reading {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "reading")
  }
}
