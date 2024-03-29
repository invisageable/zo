use super::Process;

use zhyr_builder::builder;

use zhoo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Clone, Debug)]
pub struct Building {
  pub rx: Sender<()>,
  pub tx: Receiver<Box<[u8]>>,
}

impl Process for Building {
  fn process(&self, _session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|bytecode| {
      builder::build(&bytecode).and_then(|ok| self.rx.send(ok))
    })
  }
}

impl std::fmt::Display for Building {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "building")
  }
}
