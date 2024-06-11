//! ...

use super::Process;

use zo_builder::builder;
use zo_builder::output::Output;
use zo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Debug)]
pub struct Building {
  pub rx: Sender<Output>,
  pub tx: Receiver<Box<[u8]>>,
}

impl Process for Building {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|bytecode| {
      println!("\n{bytecode:?}\n");
      builder::build(session, &bytecode).and_then(|output| self.rx.send(output))
    })
  }
}

impl std::fmt::Display for Building {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "building")
  }
}
