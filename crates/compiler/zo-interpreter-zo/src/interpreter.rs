use zo_ast::ast::{
  Ast, BinOp, BinOpKind, Expr, ExprKind, Lit, LitKind, Stmt, StmtKind, UnOp,
};

use zo_interner::interner::symbol::Symbol;
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::Result;
use zo_session::session::Session;
use zo_value::value::{Value, ValueKind};

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
      ExprKind::UnOp(unop, rhs) => {
        self.interpret_expr_unop(unop, rhs, expr.span)
      }
      ExprKind::BinOp(binop, lhs, rhs) => {
        self.interpret_expr_binop(binop, lhs, rhs, expr.span)
      }
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

  fn interpret_expr_unop(
    &mut self,
    unop: &UnOp,
    rhs: &Expr,
    span: Span,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_binop(
    &mut self,
    binop: &BinOp,
    lhs: &Expr,
    rhs: &Expr,
    span: Span,
  ) -> Result<Value> {
    let lhs = self.interpret_expr(lhs)?;
    let rhs = self.interpret_expr(rhs)?;

    match (&lhs.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        self.interpret_expr_binop_int(binop, lhs, rhs, span)
      }
      _ => todo!(),
    }
  }

  fn interpret_expr_binop_int(
    &mut self,
    binop: &BinOp,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::Add => Ok(Value::int(lhs + rhs, span)),
      _ => todo!(),
    }
  }
}

/// Evaluates an AST.
///
/// See also [`Interpreter::interpret`].
pub fn interpret(session: &mut Session, ast: &Ast) -> Result<Value> {
  Interpreter::new(&mut session.interner, &session.reporter).interpret(ast)
}
