use super::Process;

use zhoo_reader::reader;
use zhoo_session::session::Session;

use zo_core::{mpsc::sender::Sender, Result};

#[derive(Clone, Debug)]
pub struct Reading {
  pub rx: Sender<Box<[u8]>>,
}

impl Process for Reading {
  fn process(&self, session: &mut Session) -> Result<()> {
    reader::read(session).and_then(|source| self.rx.send(source))
  }
}

impl std::fmt::Display for Reading {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "reading")
  }
}
