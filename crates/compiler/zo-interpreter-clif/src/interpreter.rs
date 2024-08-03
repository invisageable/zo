use zo_ast::ast::{Ast, Stmt};
use zo_reporter::Result;
use zo_session::session::Session;
use zo_value::value::Value;

/// The representation of an interpreter.
struct Interpreter;
impl Interpreter {
  /// Evaluates the AST.
  fn interpret(&mut self, _session: &mut Session, ast: &Ast) -> Result<Value> {
    let mut value = Value::UNIT;

    for stmt in ast.iter() {
      value = match self.interpret_stmt(stmt) {
        Ok(value) => value,
        Err(_report_error) => panic!(),
      };
    }

    Ok(value)
  }

  /// Evaluates a statement.
  fn interpret_stmt(&mut self, stmt: &Stmt) -> Result<Value> {
    todo!()
  }
}

/// Evaluates the AST.
///
/// See also [`Interpreter::interpret`].
///
/// #### examples.
///
/// ```
/// use zo_ast::ast::Ast;
/// use zo_interpreter_clif::interpreter;
/// use zo_session::session::Session;
/// use zo_value::value::Value;
///
/// let mut session = Session::default();
/// let ast = Ast::new();
/// let value = interpreter::interpret(&mut session, &ast);
///
/// assert_eq!(value, Value::UNIT);
/// ```
pub fn interpret(session: &mut Session, ast: &Ast) -> Result<Value> {
  Interpreter.interpret(session, ast)
}
