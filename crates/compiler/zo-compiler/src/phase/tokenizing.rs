//! ...

use super::Process;

use zo_session::session::Session;
use zo_tokenizer::token::Token;
use zo_tokenizer::tokenizer;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Debug)]
pub struct Tokenizing {
  pub rx: Sender<Vec<Token>>,
  pub tx: Receiver<Box<[u8]>>,
}

impl Process for Tokenizing {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|source| {
      tokenizer::tokenize(session, &source).and_then(|tokens| {
        println!("\n{tokens:?}\n");
        self.rx.send(tokens)
      })
    })
  }
}

impl std::fmt::Display for Tokenizing {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "tokenizing")
  }
}
