use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::session::Session;
use zo_value::value::Value;

/// The representation of an interpreter.
struct Interpreter;
impl Interpreter {
  /// Evaluates the AST.
  fn interpret(&mut self, _session: &mut Session, ast: &Ast) -> Result<Value> {
    let mut value = Value::ZERO;

    Ok(value)
  }
}

/// Evaluates the AST.
///
/// See also [`Interpreter::interpret`].
pub fn interpret(session: &mut Session, ast: &Ast) -> Result<Value> {
  Interpreter.interpret(session, ast)
}
