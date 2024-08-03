use zo_ast::ast::{Ast, Expr, ExprKind, Lit, LitKind, Stmt, StmtKind};
use zo_interner::interner::symbol::Symbol;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;
use zo_value::value::Value;

use swisskit::span::Span;

/// The representation of an interpreter.
struct Interpreter<'ast> {
  /// See [`Interner`].
  interner: &'ast mut Interner,
  /// See [`Reporter`].
  reporter: &'ast Reporter,
}

impl<'ast> Interpreter<'ast> {
  /// Creates a new interpreter.
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  /// Evaluates an AST.
  fn interpret(&mut self, ast: &Ast) -> Result<Value> {
    let mut value = Value::UNIT;

    for stmt in ast.iter() {
      value = match self.interpret_stmt(stmt) {
        Ok(value) => value,
        Err(report_error) => self.reporter.raise(report_error),
      };
    }

    Ok(value)
  }

  /// Evaluates a statement.
  fn interpret_stmt(&mut self, stmt: &Stmt) -> Result<Value> {
    match &stmt.kind {
      StmtKind::Expr(expr) => self.interpret_stmt_expr(expr),
    }
  }

  /// Evaluates an expression statement.
  fn interpret_stmt_expr(&mut self, expr: &Expr) -> Result<Value> {
    self.interpret_expr(expr)
  }

  /// Evaluates an expression.
  fn interpret_expr(&mut self, expr: &Expr) -> Result<Value> {
    match &expr.kind {
      ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      _ => todo!(),
    }
  }

  /// Evaluates a literal expression.
  fn interpret_expr_lit(&mut self, lit: &Lit) -> Result<Value> {
    match &lit.kind {
      LitKind::Int(sym, _) => self.interpret_expr_lit_int(sym, lit.span),
      _ => todo!(),
    }
  }

  /// Evaluates an integer literal expression.
  fn interpret_expr_lit_int(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let int = self.interner.lookup_int(**sym as usize);

    Ok(Value::int(int, span))
  }
}

/// Evaluates an AST.
///
/// See also [`Interpreter::interpret`].
pub fn interpret(session: &mut Session, ast: &Ast) -> Result<Value> {
  Interpreter::new(&mut session.interner, &session.reporter).interpret(ast)
}
