use super::Process;

use zhoo_ast::ast::Program;
use zhoo_codegen::codegen;
use zhoo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Clone, Debug)]
pub struct Generating {
  pub rx: Sender<Box<[u8]>>,
  pub tx: Receiver<Program>,
}

impl Process for Generating {
  fn process(&self, _session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|_program| {
      codegen::generate().and_then(|bytecode| self.rx.send(bytecode))
    })
  }
}

impl std::fmt::Display for Generating {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "generating")
  }
}
