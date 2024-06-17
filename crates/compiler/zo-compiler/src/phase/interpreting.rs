//! ...

use super::Process;

use zo_ast::ast::Ast;
use zo_interpreter::interpreter;
use zo_interpreter::interpreter::Interpreter;
use zo_session::session::Session;
use zo_value::value::Value;

use zo_core::mpsc::receiver::Receiver;
use zo_core::mpsc::sender::Sender;
use zo_core::Result;

#[derive(Debug)]
pub struct Interpreting {
  pub rx: Sender<Value>,
  pub tx: Receiver<Ast>,
}

impl Process for Interpreting {
  fn process(&self, session: &mut Session) -> Result<()> {
    self.tx.recv().and_then(|ast| {
      let mut interpreter =
        Interpreter::new(&mut session.interner, &session.reporter);

      interpreter::interpret(&mut interpreter, &ast).and_then(|value| {
        println!("{value}");
        self.rx.send(value)
      })
    })
  }
}

impl std::fmt::Display for Interpreting {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "interpreting")
  }
}
