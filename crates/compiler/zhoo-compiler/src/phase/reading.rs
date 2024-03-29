use super::Process;

use zhoo_reader::reader;
use zhoo_session::session::Session;

use zo_core::{mpsc::sender::Sender, Result};

#[derive(Debug)]
pub struct Reading {
  pub rx: Sender<Box<[u8]>>,
}

impl Process for Reading {
  fn process(&self, session: &mut Session) -> Result<()> {
    reader::read(session).and_then(|source_bytes| self.rx.send(source_bytes))
  }
}

impl std::fmt::Display for Reading {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "reading")
  }
}
