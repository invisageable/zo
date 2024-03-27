use super::Process;

use zhyr_ast::ast::Ast;
use zhyr_parser::parser;

use zhoo_session::session::Session;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Clone, Debug)]
pub struct Parsing {
  pub rx: Sender<Ast>,
  pub tx: Receiver<Vec<std::path::PathBuf>>,
}

impl Process for Parsing {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|paths| {
      parser::parse(session, &paths).and_then(|ast| self.rx.send(ast))
    })
  }
}

impl std::fmt::Display for Parsing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "parsing")
  }
}
