use super::Process;

use zhoo_ast::ast::Program;
use zhoo_parser::parser;
use zhoo_session::session::Session;
use zhoo_tokenizer::token::Token;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Debug)]
pub struct Parsing {
  pub rx: Sender<Program>,
  pub tx: Receiver<Vec<Token>>,
}

impl Process for Parsing {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|tokens| {
      println!("\n{tokens:?}\n\nLENGTH: {}\n", tokens.len());
      parser::parse(session, &tokens).and_then(|program| self.rx.send(program))
    })
  }
}

impl std::fmt::Display for Parsing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "parsing")
  }
}
