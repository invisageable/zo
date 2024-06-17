use zo_ast::ast::{
  Args, Ast, BinOp, Block, Expr, ExprKind, Lit, Prototype, Var,
};

use zo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::{is, to, Result};

struct NameChecker<'ast> {
  interner: &'ast mut Interner,
  reporter: &'ast Reporter,
}

impl<'ast> NameChecker<'ast> {
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self { interner, reporter }
  }

  fn check(&mut self, ast: &Ast) -> Result<()> {
    for expr in &ast.exprs {
      if let Err(error) = self.check_expr(expr) {
        self.reporter.add_report(error);
      }
    }

    self.reporter.abort_if_has_errors();

    Ok(())
  }

  fn check_expr(&mut self, expr: &Expr) -> Result<()> {
    match &expr.kind {
      ExprKind::Lit(lit) => self.check_expr_lit(lit),
      ExprKind::UnOp(_, rhs) => self.check_expr_unop(rhs),
      ExprKind::BinOp(_, lhs, rhs) => self.check_expr_binop(lhs, rhs),
      ExprKind::Assign(assignee, value) => {
        self.check_expr_assgin(assignee, value)
      }
      ExprKind::AssignOp(binop, assignee, value) => {
        self.check_expr_assgin_op(binop, assignee, value)
      }
      ExprKind::Block(body) => self.check_expr_block(body),
      ExprKind::Fn(prototype, body) => self.check_expr_fn(prototype, body),
      ExprKind::Call(callee, args) => self.check_expr_call(callee, args),
      ExprKind::Array(elmts) => self.check_expr_array(elmts),
      ExprKind::ArrayAccess(indexed, index) => {
        self.check_expr_array_access(indexed, index)
      }
      ExprKind::IfElse(condition, consequence, maybe_alternative) => {
        self.check_expr_if_else(condition, consequence, maybe_alternative)
      }
      ExprKind::When(condition, consequence, maybe_alternative) => {
        self.check_expr_when(condition, consequence, maybe_alternative)
      }
      ExprKind::Loop(body) => self.check_expr_loop(body),
      ExprKind::While(condition, body) => {
        self.check_expr_while(condition, body)
      }
      ExprKind::Return(maybe_expr) => self.check_expr_return(maybe_expr),
      ExprKind::Break(maybe_expr) => self.check_expr_break(maybe_expr),
      ExprKind::Var(var) => self.check_expr_var(var),
      _ => Ok(()),
    }
  }

  fn check_expr_lit(&mut self, _lit: &Lit) -> Result<()> {
    Ok(())
  }

  fn check_expr_unop(&mut self, rhs: &Expr) -> Result<()> {
    self.check_expr(rhs)
  }

  fn check_expr_binop(&mut self, lhs: &Expr, rhs: &Expr) -> Result<()> {
    self.check_expr(lhs)?;
    self.check_expr(rhs)
  }

  fn check_expr_assgin(
    &mut self,
    _assignee: &Expr,
    _value: &Expr,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_assgin_op(
    &mut self,
    _binop: &BinOp,
    _assignee: &Expr,
    _value: &Expr,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_block(&mut self, body: &Block) -> Result<()> {
    self.check_block(body)
  }

  fn check_block(&mut self, body: &Block) -> Result<()> {
    for expr in &body.exprs {
      self.check_expr(expr)?;
    }

    Ok(())
  }

  fn check_expr_fn(
    &mut self,
    prototype: &Prototype,
    body: &Block,
  ) -> Result<()> {
    self.check_prototype(prototype)?;
    self.check_block(body)
  }

  fn check_prototype(&mut self, _prototype: &Prototype) -> Result<()> {
    todo!()
  }

  fn check_expr_call(&mut self, _callee: &Expr, _args: &Args) -> Result<()> {
    todo!()
  }

  fn check_expr_array(&mut self, _elmts: &Vec<Expr>) -> Result<()> {
    todo!()
  }

  fn check_expr_array_access(
    &mut self,
    _indexed: &Expr,
    _index: &Expr,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_if_else(
    &mut self,
    _condition: &Expr,
    _consequence: &Block,
    _maybe_alternative: &Option<Box<Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_when(
    &mut self,
    _condition: &Expr,
    _consequence: &Expr,
    _alternative: &Expr,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_loop(&mut self, body: &Block) -> Result<()> {
    self.check_block(body)
  }

  fn check_expr_while(
    &mut self,
    _condition: &Expr,
    _body: &Block,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<Expr>>,
  ) -> Result<()> {
    todo!()
  }

  fn check_expr_var(&mut self, _var: &Var) -> Result<()> {
    todo!()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn check(session: &mut Session, ast: &Ast) -> Result<()> {
  NameChecker::new(&mut session.interner, &session.reporter).check(ast)
}
