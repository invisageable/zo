use super::Process;

use zhyr_ast::ast::Ast;
use zhyr_codegen::codegen;

use zhoo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Clone, Debug)]
pub struct Generating {
  pub rx: Sender<Box<[u8]>>,
  pub tx: Receiver<Ast>,
}

impl Process for Generating {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|ast| {
      codegen::generate(session, &ast)
        .and_then(|bytecode| self.rx.send(bytecode))
    })
  }
}

impl std::fmt::Display for Generating {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "generating")
  }
}
