//! ...

use super::Process;

use zo_analyzer::analyzer;
use zo_ast::ast::Ast;
use zo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Debug)]
pub struct Analyzing {
  pub rx: Sender<Ast>,
  pub tx: Receiver<Ast>,
}

impl Process for Analyzing {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|ast| {
      analyzer::analyze(session, &ast).and_then(|_| {
        println!("\n{ast:?}\n");
        self.rx.send(ast)
      })
    })
  }
}

impl std::fmt::Display for Analyzing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "analyzing")
  }
}
