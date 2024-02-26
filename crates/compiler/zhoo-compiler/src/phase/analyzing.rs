use super::Process;

use zhoo_analyzer::analyzer;
use zhoo_ast::ast::Program;
use zhoo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Clone, Debug)]
pub struct Analyzing {
  pub rx: Sender<Program>,
  pub tx: Receiver<Program>,
}

impl Process for Analyzing {
  fn process(&self, _session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|program| {
      analyzer::analyze().and_then(|_program| self.rx.send(program))
    })
  }
}

impl std::fmt::Display for Analyzing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "analyzing")
  }
}
