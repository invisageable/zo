//! ...

use super::Process;

use zo_ast::ast::Ast;
use zo_codegen::codegen;
use zo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Debug)]
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
