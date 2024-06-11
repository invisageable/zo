//! ...

use super::Process;

use zo_ast::ast::Ast;
use zo_parser::parser;
use zo_session::session::Session;
use zo_tokenizer::token::Token;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Debug)]
pub struct Parsing {
  pub rx: Sender<Ast>,
  pub tx: Receiver<Vec<Token>>,
}

impl Process for Parsing {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|tokens| {
      parser::parse(session, &tokens).and_then(|ast| {
        println!("\n{ast:?}\n");
        self.rx.send(ast)
      })
    })
  }
}

impl std::fmt::Display for Parsing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "parsing")
  }
}
